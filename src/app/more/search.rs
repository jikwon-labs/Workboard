// Search: the results for a query (tasks, docs, people, places, messages...),
// opening a result, and the search palette.

use super::*;

/// Where a search result leads.
#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    Go(Location),
    Task(u64),
    Doc(u64),
    Chat(u64),
    /// A message: (its conversation, the message, the thread it is a reply in).
    Message(u64, u64, Option<u64>),
    Whiteboard(u64),
    Form(u64),
    Dashboard(u64),
    Goal(u64),
    Person(u64),
    /// A recent search: put it back in the box.
    Query(String),
}

/// One search result row: (group, glyph, title, place, where it leads).
pub type SearchRow = (&'static str, &'static str, String, String, Hit);

/// Kind chips over the results.
pub const SEARCH_KINDS: [&str; 8] = [
    "All", "Tasks", "Lists", "Docs", "Channels", "Messages", "Apps", "People",
];

impl App {
    /// Everything the search box finds, grouped, best first. A pasted
    /// workboard:// link (Copy link) or a task number finds that item.
    /// Archived tasks and Lists (Spaces, Folders) whose names match: search
    /// leaves them out, so it says how many. Kind as in SEARCH_KINDS.
    pub(super) fn archived_matches(&self, q: &str, kind: u8) -> usize {
        let ql = q.trim().to_lowercase();
        if ql.is_empty() || super::nav::parse_link(q).is_some() {
            return 0;
        }
        let mine = |sp: &Space| sp.private_owner.is_none_or(|o| o == self.data.me);
        let tasks = if kind == 0 || kind == 1 {
            self.data
                .tasks
                .iter()
                .filter(|t| !self.data.task_live(t))
                .filter(|t| self.data.space_of_list(t.list_id).is_some_and(mine))
                .filter(|t| t.name.to_lowercase().contains(&ql))
                .count()
        } else {
            0
        };
        let lists = if kind == 0 || kind == 2 {
            self.data
                .spaces
                .iter()
                .filter(|sp| mine(sp))
                .flat_map(|sp| {
                    sp.lists
                        .iter()
                        .chain(sp.folders.iter().flat_map(|f| f.lists.iter()))
                })
                .filter(|l| self.data.list_archived(l.id) && l.name.to_lowercase().contains(&ql))
                .count()
        } else {
            0
        };
        tasks + lists
    }

    pub(crate) fn search_rows(&self, q: &str, kind: u8) -> Vec<SearchRow> {
        let ql = q.trim().to_lowercase();
        let many = |k: u8| if kind == k { 60 } else { 8 };
        let mut rows: Vec<SearchRow> = vec![];
        // a pasted workboard:// link (from Copy link)
        if let Some(link) = super::nav::parse_link(q) {
            let hit = match link {
                super::nav::Link::Task(id) => self.task(id).map(|t| {
                    (
                        "task",
                        t.name.clone(),
                        self.data.list_name(t.list_id),
                        Hit::Task(id),
                    )
                }),
                super::nav::Link::Loc(l @ Location::List(id)) => {
                    self.data.location_exists(l).then(|| {
                        (
                            "list",
                            self.data.list_name(id),
                            self.data
                                .space_of_list(id)
                                .map(|s| s.name.clone())
                                .unwrap_or_default(),
                            Hit::Go(l),
                        )
                    })
                }
                super::nav::Link::Loc(l @ Location::Folder(_)) => {
                    self.data.location_exists(l).then(|| {
                        (
                            "folder",
                            self.data.location_name(l),
                            tr("Folder").into(),
                            Hit::Go(l),
                        )
                    })
                }
                super::nav::Link::Loc(l @ Location::Space(_)) => {
                    self.data.location_exists(l).then(|| {
                        (
                            "space",
                            self.data.location_name(l),
                            tr("Space").into(),
                            Hit::Go(l),
                        )
                    })
                }
                super::nav::Link::Loc(l) => Some((
                    "arrow_right",
                    self.data.location_name(l),
                    String::new(),
                    Hit::Go(l),
                )),
                super::nav::Link::Doc(id) => self
                    .data
                    .docs
                    .iter()
                    .find(|d| d.id == id)
                    .map(|d| ("docs", d.title.clone(), tr("Doc").into(), Hit::Doc(id))),
            };
            match hit {
                // archived, or in someone else's Personal space
                Some(_) if self.link_shown(link) == Some(false) => rows.push((
                    "Link",
                    "warning",
                    tr("This link points to something archived or private").into(),
                    String::new(),
                    Hit::Query(String::new()),
                )),
                Some((g, title, place, hit)) => rows.push(("Link", g, title, place, hit)),
                None => rows.push((
                    "Link",
                    "warning",
                    tr("Nothing with this link here (it may have been deleted)").into(),
                    String::new(),
                    Hit::Query(String::new()),
                )),
            }
            return rows;
        }
        // "has:file", "has:link" and "from:name" ask for messages only
        // (msgquery.rs); what's left is the text to find
        let mq = super::msgquery::MsgQuery::parse(q);
        let narrowed = mq.narrows();
        let show = |k: u8| (kind == 0 || kind == k) && (!narrowed || k == 5);
        let ql = if narrowed { mq.text.clone() } else { ql };
        if ql.is_empty() && kind == 0 && !narrowed {
            for (k, id, name, place) in self.recent_items(None).into_iter().take(8) {
                let (g, hit) = match k.as_str() {
                    "task" => ("task", Hit::Task(id)),
                    "doc" => ("docs", Hit::Doc(id)),
                    _ => ("list", Hit::Go(Location::List(id))),
                };
                rows.push(("Recent", g, name, place, hit));
            }
            for r in self.sweep.search_recent.iter().take(5) {
                rows.push((
                    "Recent searches",
                    "search",
                    r.clone(),
                    String::new(),
                    Hit::Query(r.clone()),
                ));
            }
        }
        let has = |s: &str| ql.is_empty() || s.to_lowercase().contains(&ql);
        let me = self.data.me;
        // like the sidebar: no other people's Personal spaces, nothing archived
        let space_shown = |s: &Space| !s.archived && s.private_owner.map_or(true, |o| o == me);
        if show(2) {
            let mut locs: Vec<SearchRow> = vec![];
            for s in self.data.spaces.iter().filter(|s| space_shown(s)) {
                locs.push((
                    "Locations",
                    "space",
                    s.name.clone(),
                    tr("Space").into(),
                    Hit::Go(Location::Space(s.id)),
                ));
                // folders and lists by their path, as in the sidebar
                for f in &s.folders {
                    locs.push((
                        "Locations",
                        "folder",
                        format!("{} / {}", s.name, f.name),
                        tr("Folder").into(),
                        Hit::Go(Location::Folder(f.id)),
                    ));
                    for l in f.lists.iter().filter(|l| !l.archived) {
                        locs.push((
                            "Locations",
                            "list",
                            ListPlace {
                                space: s,
                                folder: Some(f),
                                list: l,
                            }
                            .path(),
                            tr("List").into(),
                            Hit::Go(Location::List(l.id)),
                        ));
                    }
                }
                for l in s.lists.iter().filter(|l| !l.archived) {
                    locs.push((
                        "Locations",
                        "list",
                        ListPlace {
                            space: s,
                            folder: None,
                            list: l,
                        }
                        .path(),
                        tr("List").into(),
                        Hit::Go(Location::List(l.id)),
                    ));
                }
            }
            rows.extend(locs.into_iter().filter(|r| has(&r.2)).take(many(2)));
        }
        if show(1) {
            // not in archived lists, folders or spaces, nor other people's Personal spaces
            let tasks = self
                .data
                .tasks
                .iter()
                .filter(|x| self.task_visible(x) && (has(&x.name) || x.id.to_string() == ql))
                .take(if kind == 1 { 60 } else { 20 });
            rows.extend(tasks.map(|x| {
                (
                    "Tasks",
                    "task",
                    x.name.clone(),
                    self.data.list_name(x.list_id),
                    Hit::Task(x.id),
                )
            }));
        }
        if show(3) {
            let docs = self
                .data
                .docs
                .iter()
                .filter(|d| {
                    self.doc_shown(d)
                        && (has(&d.title) || (ql.len() > 2 && d.body.to_lowercase().contains(&ql)))
                })
                .take(many(3));
            rows.extend(docs.map(|d| {
                (
                    "Docs",
                    "docs",
                    d.title.clone(),
                    tr("Doc").into(),
                    Hit::Doc(d.id),
                )
            }));
        }
        if show(4) {
            let chans = self
                .data
                .channels
                .iter()
                .filter(|c| c.kind == ChannelKind::Channel && self.can_see(c) && has(&c.name))
                .take(many(4));
            rows.extend(chans.map(|c| {
                (
                    "Channels",
                    "#",
                    c.name.clone(),
                    tr("Channel").into(),
                    Hit::Chat(c.id),
                )
            }));
        }
        if show(5) && (ql.len() > 1 || narrowed) {
            let mut n = 0;
            'outer: for c in self.data.channels.iter().filter(|c| self.can_see(c)) {
                let place = if c.kind == ChannelKind::Direct {
                    tr("Direct message").to_string()
                } else {
                    format!("#{}", c.name)
                };
                // the messages, then the replies with the thread they are in
                let all = c.messages.iter().map(|m| (m, None)).chain(
                    c.messages
                        .iter()
                        .flat_map(|m| m.replies.iter().map(move |r| (r, Some(m.id)))),
                );
                for (m, thread) in all {
                    // a post is found by its title too (its text may be
                    // empty), a title someone wrote, not the default
                    let title = super::chat::typed_post_title(m);
                    let in_text = m.text.to_lowercase().contains(&ql);
                    let in_title = title.is_some_and(|t| t.to_lowercase().contains(&ql));
                    if (in_text || in_title) && mq.fits(m, &self.data) {
                        let shown = match title {
                            Some(t) if !in_text => t.to_string(),
                            _ => super::chat::shown_text(m),
                        };
                        rows.push((
                            "Messages",
                            "chat",
                            truncate(&one_line(&shown), 60),
                            place.clone(),
                            Hit::Message(c.id, m.id, thread),
                        ));
                        n += 1;
                        if n >= many(5) {
                            break 'outer;
                        }
                    }
                }
            }
        }
        if show(6) {
            let x = &self.data.extra;
            let mut apps: Vec<SearchRow> = vec![];
            apps.extend(x.whiteboards.iter().filter(|w| has(&w.name)).map(|w| {
                (
                    "Apps",
                    "whiteboards",
                    w.name.clone(),
                    tr("Whiteboard").into(),
                    Hit::Whiteboard(w.id),
                )
            }));
            apps.extend(
                x.forms
                    .iter()
                    .filter(|f| self.form_shown(f) && has(&f.name))
                    .map(|f| {
                        (
                            "Apps",
                            "forms",
                            f.name.clone(),
                            tr("Form").into(),
                            Hit::Form(f.id),
                        )
                    }),
            );
            apps.extend(
                x.dashboards
                    .iter()
                    .filter(|d| self.dash_shown(d) && has(&d.name))
                    .map(|d| {
                        (
                            "Apps",
                            "dashboards",
                            d.name.clone(),
                            tr("Dashboard").into(),
                            Hit::Dashboard(d.id),
                        )
                    }),
            );
            apps.extend(x.goals.iter().filter(|g| has(&g.name)).map(|g| {
                (
                    "Apps",
                    "goals",
                    g.name.clone(),
                    tr("Goal").into(),
                    Hit::Goal(g.id),
                )
            }));
            if !(ql.is_empty() && kind == 0) {
                rows.extend(apps.into_iter().take(many(6)));
            }
        }
        if show(7) && !(ql.is_empty() && kind == 0) {
            let people = self
                .data
                .members
                .iter()
                .filter(|m| {
                    m.any_name(&has) || (!ql.is_empty() && m.email.to_lowercase().contains(&ql))
                })
                .take(many(7));
            rows.extend(people.map(|m| {
                (
                    "People",
                    "person",
                    m.shown().to_string(),
                    if m.email.is_empty() {
                        super::menus::role_text(&m.role).to_string()
                    } else {
                        m.email.clone()
                    },
                    Hit::Person(m.id),
                )
            }));
        }
        rows
    }

    /// Goes where a search result leads.
    pub(super) fn open_hit(&mut self, hit: Hit) {
        match hit {
            Hit::Go(l) => self.actions.push(Action::Go(l)),
            Hit::Task(id) => self.actions.push(Action::Open(id)),
            Hit::Doc(id) => {
                self.doc_open = Some(id);
                self.section = Section::Docs;
            }
            Hit::Chat(id) => {
                self.section = Section::Chat;
                self.chat.open = Some(id);
                self.leave_thread();
            }
            // at the message, as "Open in channel" on a post opens one: a
            // reply in its thread, nothing filtered out of the list
            Hit::Message(cid, mid, thread) => {
                self.open_chat_at(cid, thread);
                self.chat.jump_to = Some(thread.unwrap_or(mid));
                self.channel_find = None;
                self.channel_panel = 0;
            }
            Hit::Whiteboard(id) => {
                self.sweep.wb.open = Some(id);
                self.section = Section::Whiteboards;
            }
            Hit::Form(id) => {
                // on Edit, as the Forms sidebar and table open one
                self.form_open = Some(id);
                self.form_tab = super::FormTab::Edit;
                self.section = Section::Forms;
            }
            Hit::Dashboard(id) => {
                self.dash_open = Some(id);
                self.section = Section::Dashboards;
            }
            Hit::Goal(id) => {
                self.goal_open = Some(id);
                self.section = Section::Goals;
            }
            Hit::Person(_) => self.section = Section::People,
            Hit::Query(_) => {}
        }
    }

    /// Cmd+K / Cmd+J search: arrow keys pick a result, Enter opens it.
    pub(crate) fn search_palette(&mut self, ctx: &egui::Context) {
        let Some(mut q) = self.palette.take() else {
            return;
        };
        let t = self.t();
        let mut open = true;
        egui::Window::new("search_palette")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_TOP, [0.0, 60.0])
            .fixed_size([620.0, 420.0])
            .show(ctx, |ui| {
                // the arrows move the highlight, not the text cursor
                let (down, up) = ui.input_mut(|i| {
                    (
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                    )
                });
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let r = ui.add(
                    field(&mut q)
                        .hint_text(tr("Search, or paste a link to a task, list or space..."))
                        .font(FontId::proportional(16.0))
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                let kid = egui::Id::new("palette_kind");
                let mut kind: u8 = ui.data(|d| d.get_temp(kid)).unwrap_or(0);
                ui.horizontal(|ui| {
                    for (i, label) in SEARCH_KINDS.iter().enumerate() {
                        if ui.selectable_label(kind == i as u8, tr(label)).clicked() {
                            kind = i as u8;
                        }
                    }
                });
                ui.data_mut(|d| d.insert_temp(kid, kind));
                ui.separator();
                let rows = self.search_rows(&q, kind);
                let key = format!("{}\u{1}{}", kind, q);
                if self.sweep.search_q != key {
                    self.sweep.search_q = key;
                    self.sweep.search_sel = 0;
                }
                let pickable: Vec<usize> = (0..rows.len())
                    .filter(|i| rows[*i].4 != Hit::Query(String::new()))
                    .collect();
                if !pickable.is_empty() {
                    let pos = pickable
                        .iter()
                        .position(|i| *i == self.sweep.search_sel)
                        .unwrap_or(0);
                    let pos = if down {
                        (pos + 1).min(pickable.len() - 1)
                    } else if up {
                        pos.saturating_sub(1)
                    } else {
                        pos
                    };
                    self.sweep.search_sel = pickable[pos];
                }
                let mut chosen: Option<Hit> = None;
                egui::ScrollArea::vertical()
                    .max_height(340.0)
                    .show(ui, |ui| {
                        let mut group = "";
                        for (i, (g, glyph, title, place, hit)) in rows.iter().enumerate() {
                            if *g != group {
                                if !group.is_empty() {
                                    ui.add_space(4.0);
                                }
                                group = g;
                                ui.label(RichText::new(tr(g)).small().color(t.text3));
                            }
                            let w = ui.available_width();
                            let (rr, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
                            name_it(&resp, title);
                            let sel = i == self.sweep.search_sel && pickable.contains(&i);
                            if sel || resp.hovered() {
                                ui.painter().rect_filled(
                                    rr,
                                    6.0,
                                    if sel { t.row_selected } else { t.hover },
                                );
                            }
                            if sel && (down || up) {
                                resp.scroll_to_me(None);
                            }
                            // a Space shows its colour and first letter as in the
                            // sidebar; everything else its kind's line icon
                            let ic = rr.left_center() + vec2(14.0, 0.0);
                            let space = match hit {
                                Hit::Go(Location::Space(id)) => {
                                    self.data.spaces.iter().find(|s| s.id == *id)
                                }
                                _ => None,
                            };
                            if let Some(sp) = space {
                                let r = Rect::from_center_size(ic, vec2(16.0, 16.0));
                                ui.painter().rect_filled(r, 4.0, rgb(sp.color));
                                ui.painter().text(
                                    r.center(),
                                    Align2::CENTER_CENTER,
                                    sp.name
                                        .chars()
                                        .next()
                                        .unwrap_or('S')
                                        .to_uppercase()
                                        .to_string(),
                                    FontId::proportional(10.0),
                                    Color32::WHITE,
                                );
                            } else {
                                line_icon(ui.painter(), glyph, ic, t.text2);
                            }
                            // the title keeps all but 200 for the grey place after
                            // it; both are shortened with an ellipsis
                            let title_r = paint_fit(
                                ui,
                                pos2(rr.left() + 28.0, rr.center().y),
                                Align2::LEFT_CENTER,
                                title,
                                FontId::proportional(14.0),
                                t.text,
                                w - 200.0,
                            );
                            paint_fit(
                                ui,
                                pos2(title_r.right() + 12.0, rr.center().y),
                                Align2::LEFT_CENTER,
                                place,
                                FontId::proportional(12.0),
                                t.text3,
                                rr.right() - 8.0 - (title_r.right() + 12.0),
                            );
                            if resp.clicked() {
                                chosen = Some(hit.clone());
                            }
                        }
                        if rows.is_empty() && !q.trim().is_empty() {
                            ui.add_space(14.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(tr("No results")).color(t.text3));
                            });
                            ui.add_space(14.0);
                        }
                        // the words that narrow messages, where they help
                        if (rows.is_empty() && !q.trim().is_empty())
                            || (kind == 5 && q.trim().is_empty())
                        {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(tr(
                                        "Narrow messages with has:file, has:link or from:name",
                                    ))
                                    .small()
                                    .color(t.text3),
                                );
                            });
                            ui.add_space(8.0);
                        }
                        // archived ones are left out: say so rather than drop them silently
                        let arch = self.archived_matches(&q, kind);
                        if arch > 0 {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(if arch == 1 {
                                    tr("1 archived match is not shown").to_string()
                                } else {
                                    trf!("{} archived matches are not shown", arch)
                                })
                                .small()
                                .color(t.text3),
                            );
                        }
                    });
                if enter && chosen.is_none() {
                    if let Some(i) = pickable.iter().find(|i| **i == self.sweep.search_sel) {
                        chosen = Some(rows[*i].4.clone());
                    }
                    if chosen.is_none() {
                        open = false;
                    }
                }
                match chosen {
                    Some(Hit::Query(text)) => {
                        q = text;
                    }
                    Some(hit) => {
                        let typed = q.trim().to_string();
                        if !typed.is_empty() && super::nav::parse_link(&typed).is_none() {
                            self.sweep.search_recent.retain(|x| x != &typed);
                            self.sweep.search_recent.insert(0, typed.clone());
                            self.sweep.search_recent.truncate(8);
                        }
                        // where it leads, the searched words light up a moment
                        // (only the words to find, not has:file or from:name)
                        let mq = super::msgquery::MsgQuery::parse(&typed);
                        let words = if super::nav::parse_link(&typed).is_some() {
                            ""
                        } else if mq.narrows() {
                            mq.text.as_str()
                        } else {
                            typed.as_str()
                        };
                        crate::find_flash::start(ui.ctx(), words);
                        self.open_hit(hit);
                        open = false;
                    }
                    None => {}
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    open = false;
                }
            });
        if open {
            self.palette = Some(q);
        }
    }
}
