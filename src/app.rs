// The application window: global top bar, sidebar hierarchy, view tabs
// (List, Board, Calendar, Gantt, Table), Home and Inbox pages, the task
// detail window, quick add and the command search.

use crate::model::*;
use crate::theme::{self, rgb, Theme};
use crate::widgets::*;
use chrono::{Datelike, Duration, NaiveDate};
use egui::{pos2, vec2, Align, Align2, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, Ui};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

mod chat;
mod docs;
mod files;
mod hubs;
mod huddle;
mod listview;
mod menus;
mod more;
mod pages;
mod parity;
mod comments;
mod settings;
mod shell;
mod together;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod ui_tests;
pub use menus::Modal;
pub use pages::Section;
use parity::TaskFilter;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    Overview,
    List,
    Board,
    Calendar,
    Gantt,
    Table,
}

impl ViewKind {
    const ALL: [ViewKind; 6] = [ViewKind::Overview, ViewKind::List, ViewKind::Board, ViewKind::Calendar, ViewKind::Gantt, ViewKind::Table];
    fn label(self) -> &'static str {
        match self {
            ViewKind::Overview => "Overview",
            ViewKind::List => "List",
            ViewKind::Board => "Board",
            ViewKind::Calendar => "Calendar",
            ViewKind::Gantt => "Gantt",
            ViewKind::Table => "Table",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            ViewKind::Overview => "\u{2139}",
            ViewKind::List => "\u{2630}",
            ViewKind::Board => "\u{25A6}",
            ViewKind::Calendar => "\u{1F4C5}",
            ViewKind::Gantt => "\u{2261}",
            ViewKind::Table => "\u{25A4}",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum GroupBy {
    Status,
    Priority,
    Assignee,
    DueDate,
    None,
    Tags,
    List,
}

impl GroupBy {
    const ALL: [GroupBy; 7] = [GroupBy::Status, GroupBy::Priority, GroupBy::Assignee, GroupBy::DueDate, GroupBy::Tags, GroupBy::List, GroupBy::None];
    fn label(self) -> &'static str {
        match self {
            GroupBy::Status => "Status",
            GroupBy::Priority => "Priority",
            GroupBy::Assignee => "Assignee",
            GroupBy::DueDate => "Due date",
            GroupBy::None => "None",
            GroupBy::Tags => "Tags",
            GroupBy::List => "List",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum SortBy {
    Manual,
    DueDate,
    Priority,
    Name,
    Created,
    Status,
    Assignee,
    Updated,
    StartDate,
    Estimate,
}

impl SortBy {
    const ALL: [SortBy; 10] = [SortBy::Manual, SortBy::Status, SortBy::Name, SortBy::Assignee, SortBy::Priority, SortBy::DueDate, SortBy::StartDate, SortBy::Created, SortBy::Updated, SortBy::Estimate];
    fn label(self) -> &'static str {
        match self {
            SortBy::Manual => "Manual",
            SortBy::DueDate => "Due date",
            SortBy::Priority => "Priority",
            SortBy::Name => "Task name",
            SortBy::Created => "Date created",
            SortBy::Status => "Status",
            SortBy::Assignee => "Assignee",
            SortBy::Updated => "Date updated",
            SortBy::StartDate => "Start date",
            SortBy::Estimate => "Time estimate",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Space(u64),
    Folder(u64),
    List(u64),
    Task(u64),
    Workspace,
}

pub enum Action {
    Open(u64),
    Replace(Task),
    Add { list: u64, name: String, status: Option<String>, priority: u8, assignees: Vec<u64>, due: Option<NaiveDate>, parent: Option<u64>, open: bool },
    DeleteTask(u64),
    Go(Location),
    ToggleSpace(u64),
    ToggleFolder(u64),
    NewSpace,
    NewFolder(u64),
    NewList { space: u64, folder: Option<u64> },
    StartRename(Target),
    AskDelete(Target),
    ToggleTimer(u64),
    ToggleFavorite(Location),
    EditStatuses(u64),
    SetSpaceColor(u64, [u8; 3]),
}

#[derive(Clone)]
struct Group {
    key: String,
    label: String,
    color: Color32,
    status: Option<Status>,
    priority: Option<u8>,
    assignee: Option<u64>,
    due: Option<NaiveDate>,
    tasks: Vec<u64>,
}

#[derive(Default)]
struct QuickAdd {
    name: String,
    list: u64,
    due: Option<NaiveDate>,
    priority: u8,
    assignees: Vec<u64>,
    description: String,
    /// None = the list's first status.
    status: Option<String>,
    tags: Vec<String>,
    tag_text: String,
}

/// What a draft holds (text, or pictures waiting to go) and whether it's empty.
trait Blank: Default {
    fn blank(&self) -> bool;
}

impl Blank for String {
    fn blank(&self) -> bool {
        self.is_empty()
    }
}

impl<T> Blank for Vec<T> {
    fn blank(&self) -> bool {
        self.is_empty()
    }
}

/// Unsent text (or attachments) per place: `swap` puts the current one away
/// when the place changes and brings back what was there before.
struct DraftBox<T = String> {
    on: Option<u64>,
    saved: HashMap<u64, T>,
}

impl<T> Default for DraftBox<T> {
    fn default() -> Self {
        DraftBox { on: None, saved: HashMap::new() }
    }
}

impl<T: Blank> DraftBox<T> {
    fn swap(&mut self, now: Option<u64>, text: &mut T) {
        if self.on == now {
            return;
        }
        if let Some(old) = self.on {
            if text.blank() {
                self.saved.remove(&old);
            } else {
                self.saved.insert(old, std::mem::take(text));
            }
        }
        *text = now.and_then(|id| self.saved.remove(&id)).unwrap_or_default();
        self.on = now;
    }
}

#[derive(Default)]
struct Drafts {
    chat: DraftBox,
    thread: DraftBox,
    chat_files: DraftBox<Vec<files::Pending>>,
    thread_files: DraftBox<Vec<files::Pending>>,
}

pub struct App {
    /// A popup or menu was open when this pass began (egui closes them on
    /// Escape partway through the pass, so later checks see them gone).
    menu_was_open: bool,
    /// A text field had keyboard focus at the end of the previous pass.
    typing_was_on: bool,
    data: Data,
    loc: Location,
    view: ViewKind,
    open_task: Option<u64>,
    search: String,
    me_mode: bool,
    show_closed: bool,
    group_by: GroupBy,
    sort_by: SortBy,
    collapsed: HashSet<String>,
    expanded_tasks: HashSet<u64>,
    adding: Option<(String, String)>,
    cal_month: NaiveDate,
    rename: Option<(Target, String)>,
    confirm_delete: Option<Target>,
    edit_statuses: Option<u64>,
    people_open: bool,
    new_person: String,
    palette: Option<String>,
    quick_add: Option<QuickAdd>,
    comment_text: String,
    sub_text: String,
    check_text: String,
    tag_text: String,
    dirty: bool,
    persist: bool,
    screenshot: Option<PathBuf>,
    frames: u32,
    shot_sent: bool,
    /// Ctrl+Option+Cmd+S: save a picture of the window next to the data file.
    snap_pending: bool,
    actions: Vec<Action>,
    home_collapsed: HashSet<&'static str>,
    section: Section,
    recent: Vec<u64>,
    doc_open: Option<u64>,
    planner_week: NaiveDate,
    sheet_week: NaiveDate,
    chat_open: Option<u64>,
    nav_last: Option<(Section, Location)>,
    nav_back: Vec<(Section, Location)>,
    nav_fwd: Vec<(Section, Location)>,
    nav_skip: bool,
    assignee_filter: Option<u64>,
    show_search: bool,
    // focus the toolbar search box once, on the frame it opens
    search_focus: bool,
    side_search_focus: bool,
    subtasks_open: bool,
    modal: Option<Modal>,
    notepad_open: bool,
    toast: Option<(String, i64)>,
    sync_rt: together::SyncRt,
    huddle: huddle::HuddleRt,
    /// Where each workspace file lives (team server, shared folder, this computer), read once.
    ws_places: HashMap<String, &'static str>,
    create_text: String,
    new_message_open: bool,
    pending_template: Option<TaskTemplate>,
    pending_description: Option<String>,
    /// Tags for the next task created from the Create Task window.
    pending_tags: Vec<String>,
    wb_open: Option<u64>,
    wb_tool: u8,
    wb_color: [u8; 3],
    form_open: Option<u64>,
    form_fill: HashMap<usize, String>,
    form_mode_fill: bool,
    dash_open: Option<u64>,
    meeting_open: Option<u64>,
    goal_open: Option<u64>,
    post_channel: u64,
    post_title: String,
    post_body: String,
    ws_file: String,
    side_filter: Option<&'static str>,
    side_search: Option<String>,
    sidebar_collapsed: bool,
    force_menu: Option<String>,
    post_type: String,
    inbox_tab: u8,
    channel_find: Option<String>,
    channel_panel: u8,
    hub_filter: u8,
    temp_app: Option<Section>,
    team_open: Option<u64>,
    sheet_entries: bool,
    sheet_rows: Vec<u64>,
    composer_mode: u8,
    post_sort_old: bool,
    my_tasks_open: bool,
    docs_sort: u8,
    docs_mine: bool,
    sheet_force: bool,
    post_following: bool,
    post_f_channel: Option<u64>,
    post_f_author: Option<u64>,
    post_f_type: Option<String>,
    inbox_filter: u8,
    /// The message being written in the open channel (`chat_open`).
    chat_text: String,
    thread_open: Option<u64>,
    /// The reply being written in the open thread (`thread_open`).
    thread_text: String,
    /// The message being changed in place: (channel, message, new text).
    chat_edit: Option<(u64, u64, String)>,
    /// A message waiting for a second click on "Delete message".
    chat_delete: Option<u64>,
    /// A channel marked unread while open: it stays unread until left.
    chat_keep_unread: Option<u64>,
    /// Unsent messages and replies of the channels and threads not on
    /// screen, by channel / message id (kept until the app quits).
    drafts: Drafts,
    /// Pictures and videos waiting in the channel and thread message boxes.
    chat_files: Vec<files::Pending>,
    thread_files: Vec<files::Pending>,
    files: files::Files,
    /// A picture or video opened large.
    viewer: Option<files::Viewer>,
    /// Files or a picture pasted into a message box (see raw_input_hook).
    pasted: Option<crate::media::Pasted>,
    /// egui pasted text for the Cmd+V being held.
    paste_text_seen: bool,
    new_channel: Option<String>,
    /// Tasks ticked for bulk editing.
    selected: HashSet<u64>,
    filter: TaskFilter,
    hidden_cols: HashSet<String>,
    /// The Fields panel (toolbar Columns) is open.
    fields_open: bool,
    /// Width of the List's rows last drawn (to tell which fields fit).
    list_width: f32,
    /// The Filter panel (toolbar Filter) is open.
    filter_open: bool,
    /// Group and "Date created" column to restore when leaving the Personal
    /// List (it shows Group: None and Date created, as in the original).
    personal_saved: Option<(GroupBy, bool)>,
    sort_desc: bool,
    cal_week: bool,
    /// Gantt scale: 0 days, 1 weeks, 2 months.
    gantt_zoom: u8,
    gantt_jump: bool,
    doc_preview: bool,
    settings_tab: u8,
    settings_state: Option<Modal>,
    /// Unsent comment text of tasks other than the one `comment_text` is for.
    comment_drafts: HashMap<u64, String>,
    /// The task `comment_text` belongs to.
    comment_for: Option<u64>,
    /// The Docs hub's own search box and tag filter (not the task list's).
    docs_search: String,
    docs_tag: Option<String>,
    /// View settings per location (see listview.rs), and the location they
    /// were last brought in for with what was showing then.
    view_prefs: HashMap<String, listview::ViewPrefs>,
    view_prefs_at: Option<(String, listview::ViewPrefs)>,
    /// The last row clicked into the selection (Shift-click selects from it).
    select_anchor: Option<u64>,
    /// The row the Up / Down keys highlight (Enter opens it).
    row_focus: Option<u64>,
    /// Rows of the List in the order drawn last frame.
    row_order: Vec<u64>,
    /// Rows and groups drawn this frame (for dropping a dragged row).
    row_spots: Vec<listview::RowSpot>,
    row_groups: Vec<Group>,
    /// The List row being dragged to a new place.
    row_drag: Option<u64>,
    /// A List row being renamed in place: (task, new name).
    row_rename: Option<(u64, String)>,
    /// The inline new-subtask field: (parent, name).
    sub_adding: Option<(u64, String)>,
    /// Trash entries of the tasks just deleted, for the Undo bar.
    undo_trash: Option<(Vec<u64>, i64)>,
    /// Asking before deleting the selected tasks.
    confirm_bulk_delete: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        setup_fonts(&cc.egui_ctx);
        Self::build(screenshot, None)
    }

    /// Builds the app state; `data` is used as-is (and never saved) when given.
    pub(crate) fn build(screenshot: Option<PathBuf>, data: Option<Data>) -> Self {
        let persist = screenshot.is_none() && data.is_none();
        // reopen the workspace that was open last (a joined team workspace, say)
        let last = if persist { Data::last_workspace().filter(|f| f != "data.json") } else { None };
        let mut ws_file = "data.json".to_string();
        let data = match data {
            Some(d) => d,
            None if persist => match last.as_deref().and_then(|f| Data::load_file(f).map(|d| (f, d))) {
                Some((f, d)) => {
                    ws_file = f.to_string();
                    d
                }
                None => Data::load(),
            },
            None => Data::fresh(),
        };
        let mut data = data;
        data.fix_activity_ids();
        let loc = data.first_list().map(Location::List).unwrap_or(Location::Home);
        let t = today();
        let args: Vec<String> = std::env::args().collect();
        let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
        let view = match arg("--view").as_deref() {
            Some("board") => ViewKind::Board,
            Some("calendar") => ViewKind::Calendar,
            Some("gantt") => ViewKind::Gantt,
            Some("table") => ViewKind::Table,
            _ => ViewKind::List,
        };
        let loc = match arg("--page").as_deref() {
            Some("home") => Location::Home,
            Some("inbox") => Location::Inbox,
            Some("everything") => Location::Everything,
            _ => loc,
        };
        let open_task = if args.iter().any(|a| a == "--open-task") { data.tasks.first().map(|t| t.id) } else { None };
        let start_modal = match arg("--modal").as_deref() {
            Some("invite") => Some(Modal::Invite { emails: String::new(), role: "Member".into() }),
            Some("automations") => Some(Modal::Automations { status: "complete".into(), action: "comment".into(), value: String::new() }),
            Some("fields") => Some(Modal::CustomFields { name: String::new(), kind: "text".into(), opts: String::new() }),
            Some("themes") => Some(Modal::Themes),
            Some("together") => Some(Modal::Together { path: String::new(), addr: String::new(), code: String::new(), result: String::new() }),
            Some("call") => Some(Modal::Call { ch: data.channels.first().map(|c| c.id).unwrap_or(0), link: String::new() }),
            Some("reminder") => Some(Modal::Reminder { text: String::new(), date: Some(today()), time: "09:00".into() }),
            _ => None,
        };
        App {
            data,
            loc,
            view,
            open_task,
            search: String::new(),
            me_mode: false,
            show_closed: false,
            group_by: GroupBy::Status,
            sort_by: SortBy::Manual,
            collapsed: HashSet::new(),
            expanded_tasks: HashSet::new(),
            adding: None,
            cal_month: NaiveDate::from_ymd_opt(t.year(), t.month(), 1).unwrap(),
            rename: None,
            confirm_delete: None,
            edit_statuses: None,
            people_open: false,
            new_person: String::new(),
            palette: None,
            quick_add: None,
            comment_text: String::new(),
            sub_text: String::new(),
            check_text: String::new(),
            tag_text: String::new(),
            dirty: false,
            persist,
            screenshot,
            frames: 0,
            shot_sent: false,
            snap_pending: false,
            actions: vec![],
            home_collapsed: ["Done"].into_iter().collect(),
            section: match arg("--section").as_deref() {
                Some("planner") => Section::Planner,
                Some("chat") => Section::Chat,
                Some("teams") => Section::Teams,
                Some("docs") => Section::Docs,
                Some("dashboards") => Section::Dashboards,
                Some("timesheets") => Section::Timesheets,
                Some("people") => Section::People,
                Some("goals") => Section::Goals,
                Some("forms") => Section::Forms,
                Some("whiteboards") => Section::Whiteboards,
                Some("meetings") => Section::Meetings,
                Some("posts") => Section::Posts,
                _ => Section::Home,
            },
            recent: vec![],
            doc_open: None,
            planner_week: t - Duration::days(t.weekday().num_days_from_sunday() as i64),
            sheet_week: t - Duration::days(t.weekday().num_days_from_sunday() as i64),
            chat_open: None,
            nav_last: None,
            nav_back: vec![],
            nav_fwd: vec![],
            nav_skip: false,
            assignee_filter: None,
            show_search: false,
            search_focus: false,
            side_search_focus: false,
            subtasks_open: false,
            modal: start_modal,
            notepad_open: false,
            toast: None,
            sync_rt: Default::default(),
            huddle: Default::default(),
            ws_places: HashMap::new(),
            create_text: String::new(),
            new_message_open: false,
            pending_template: None,
            pending_description: None,
            pending_tags: vec![],
            wb_open: None,
            wb_tool: 0,
            wb_color: [32, 32, 32],
            form_open: None,
            form_fill: HashMap::new(),
            form_mode_fill: false,
            dash_open: None,
            meeting_open: None,
            goal_open: None,
            post_channel: 0,
            post_title: String::new(),
            post_body: String::new(),
            ws_file,
            side_filter: None,
            side_search: None,
            sidebar_collapsed: false,
            force_menu: arg("--menu"),
            post_type: "Announcement".into(),
            inbox_tab: 0,
            channel_find: None,
            channel_panel: 0,
            hub_filter: 0,
            temp_app: None,
            team_open: None,
            sheet_entries: false,
            sheet_rows: vec![],
            composer_mode: 0,
            post_sort_old: false,
            my_tasks_open: true,
            docs_sort: 0,
            docs_mine: false,
            sheet_force: false,
            post_following: false,
            post_f_channel: None,
            post_f_author: None,
            post_f_type: None,
            inbox_filter: 0,
            chat_text: String::new(),
            thread_open: None,
            thread_text: String::new(),
            chat_edit: None,
            chat_delete: None,
            chat_keep_unread: None,
            drafts: Drafts::default(),
            chat_files: vec![],
            thread_files: vec![],
            files: files::Files::new(if persist { Data::dir().join("files") } else { std::env::temp_dir().join(format!("wb-files-{:x}", wb_sync::random_u64())) }),
            viewer: None,
            pasted: None,
            paste_text_seen: false,
            new_channel: None,
            selected: HashSet::new(),
            menu_was_open: false,
            typing_was_on: false,
            filter: TaskFilter::default(),
            hidden_cols: std::iter::once("Date created").chain(parity::PROPERTIES).map(String::from).collect(),
            fields_open: false,
            list_width: 0.0,
            filter_open: false,
            personal_saved: None,
            sort_desc: false,
            cal_week: false,
            gantt_zoom: 0,
            gantt_jump: true,
            doc_preview: false,
            settings_tab: 0,
            settings_state: None,
            comment_drafts: HashMap::new(),
            comment_for: None,
            docs_search: String::new(),
            docs_tag: None,
            view_prefs: if persist { listview::load_prefs() } else { HashMap::new() },
            view_prefs_at: None,
            select_anchor: None,
            row_focus: None,
            row_order: vec![],
            row_spots: vec![],
            row_groups: vec![],
            row_drag: None,
            row_rename: None,
            sub_adding: None,
            undo_trash: None,
            confirm_bulk_delete: false,
        }
    }

    fn t(&self) -> Theme {
        theme::theme_accent(self.data.dark, &self.data.extra.accent)
    }

    // ---------------------------------------------------------------- data helpers

    fn task(&self, id: u64) -> Option<&Task> {
        self.data.tasks.iter().find(|t| t.id == id)
    }

    fn subtasks(&self, id: u64) -> Vec<&Task> {
        let mut v: Vec<&Task> = self.data.tasks.iter().filter(|t| t.parent == Some(id)).collect();
        v.sort_by_key(|t| t.order);
        v
    }

    /// Tasks matching the current location and filters (top level only,
    /// plus the subtasks a search finds).
    fn filtered(&self) -> Vec<&Task> {
        let lists = self.data.lists_in(self.loc);
        let q = self.search.to_lowercase();
        let mut v: Vec<&Task> = self
            .data
            .tasks
            .iter()
            .filter(|t| t.parent.is_none() || !q.is_empty())
            .filter(|t| match self.loc {
                Location::MyTasks => t.assignees.contains(&self.data.me),
                Location::TodayOverdue => t.assignees.contains(&self.data.me) && t.due.map(|d| d <= today()).unwrap_or(false) && !self.data.is_closed(t),
                _ => lists.as_ref().map(|l| l.contains(&t.list_id)).unwrap_or(true),
            })
            .filter(|t| q.is_empty() || t.name.to_lowercase().contains(&q) || t.tags.iter().any(|g| g.to_lowercase().contains(&q)))
            .filter(|t| !self.me_mode || t.assignees.contains(&self.data.me))
            .filter(|t| self.data.space_of_list(t.list_id).and_then(|s| s.private_owner).map(|o| o == self.data.me).unwrap_or(true))
            .filter(|t| matches!(self.loc, Location::List(_)) || !self.data.list_archived(t.list_id))
            .filter(|t| self.assignee_filter.map(|a| t.assignees.contains(&a)).unwrap_or(true))
            .filter(|t| self.show_closed || self.filter.archived || !self.data.is_closed(t))
            .filter(|t| self.filter.matches(t, &self.data))
            .collect();
        self.sort(&mut v);
        v
    }

    fn sort(&self, v: &mut [&Task]) {
        match self.sort_by {
            SortBy::Manual => v.sort_by_key(|t| t.order),
            SortBy::DueDate => v.sort_by_key(|t| (t.due.is_none(), t.due, t.order)),
            SortBy::Priority => v.sort_by_key(|t| (if t.priority == 0 { 9 } else { t.priority }, t.order)),
            SortBy::Name => v.sort_by_key(|t| t.name.to_lowercase()),
            SortBy::Created => v.sort_by_key(|t| std::cmp::Reverse(t.created)),
            SortBy::Updated => v.sort_by_key(|t| std::cmp::Reverse(t.updated)),
            SortBy::StartDate => v.sort_by_key(|t| (t.start.is_none(), t.start, t.order)),
            SortBy::Estimate => v.sort_by_key(|t| (t.estimate_min.is_none(), t.estimate_min, t.order)),
            SortBy::Status => {
                // in the order the statuses are set up (open first, closed last)
                let order: Vec<String> = self.location_statuses().into_iter().map(|s| s.name).collect();
                v.sort_by_key(|t| (order.iter().position(|n| *n == self.data.status_of(t).name).unwrap_or(usize::MAX), t.order));
            }
            SortBy::Assignee => {
                // by the first assignee's name; unassigned last
                let key = |t: &Task| t.assignees.iter().filter_map(|a| self.data.member(*a)).map(|m| m.name.to_lowercase()).min();
                v.sort_by_key(|t| (key(t).is_none(), key(t), t.order));
            }
        }
        if self.sort_desc && self.sort_by != SortBy::Manual {
            v.reverse();
        }
    }

    /// Statuses shown for the current location, in order (union across spaces).
    fn location_statuses(&self) -> Vec<Status> {
        let spaces: Vec<&Space> = match self.loc {
            Location::List(id) => self.data.space_of_list(id).into_iter().collect(),
            Location::Folder(fid) => self.data.spaces.iter().filter(|s| s.folders.iter().any(|f| f.id == fid)).collect(),
            Location::Space(sid) => self.data.spaces.iter().filter(|s| s.id == sid).collect(),
            _ => self.data.spaces.iter().collect(),
        };
        let mut out: Vec<Status> = vec![];
        for s in spaces {
            for st in &s.statuses {
                if !out.iter().any(|o| o.name == st.name) {
                    out.push(st.clone());
                }
            }
        }
        if out.is_empty() {
            out = default_statuses();
        }
        // closed statuses last
        out.sort_by_key(|s| match s.kind {
            StatusKind::Open => 0,
            StatusKind::Custom => 1,
            StatusKind::Done => 2,
            StatusKind::Closed => 3,
        });
        out
    }

    fn groups(&self, tasks: &[&Task], include_empty: bool) -> Vec<Group> {
        let t = self.t();
        let mut out: Vec<Group> = vec![];
        match self.group_by {
            GroupBy::Status => {
                for st in self.location_statuses() {
                    if !self.show_closed && matches!(st.kind, StatusKind::Closed | StatusKind::Done) && !include_empty {
                        continue;
                    }
                    let ids: Vec<u64> = tasks.iter().filter(|x| self.data.status_of(x).name == st.name).map(|x| x.id).collect();
                    if ids.is_empty() && !include_empty {
                        continue;
                    }
                    out.push(Group { key: format!("s:{}", st.name), label: st.name.to_uppercase(), color: rgb(st.color), status: Some(st), priority: None, assignee: None, due: None, tasks: ids });
                }
            }
            GroupBy::Priority => {
                for pr in [1u8, 2, 3, 4, 0] {
                    let ids: Vec<u64> = tasks.iter().filter(|x| x.priority == pr).map(|x| x.id).collect();
                    if ids.is_empty() && !include_empty {
                        continue;
                    }
                    out.push(Group { key: format!("p:{}", pr), label: PRIORITIES[pr as usize].label.to_uppercase(), color: priority_color(pr, &t), status: None, priority: Some(pr), assignee: None, due: None, tasks: ids });
                }
            }
            GroupBy::Assignee => {
                for m in &self.data.members {
                    let ids: Vec<u64> = tasks.iter().filter(|x| x.assignees.contains(&m.id)).map(|x| x.id).collect();
                    if ids.is_empty() && !include_empty {
                        continue;
                    }
                    out.push(Group { key: format!("a:{}", m.id), label: m.name.to_uppercase(), color: rgb(m.color), status: None, priority: None, assignee: Some(m.id), due: None, tasks: ids });
                }
                let ids: Vec<u64> = tasks.iter().filter(|x| x.assignees.is_empty()).map(|x| x.id).collect();
                if !ids.is_empty() || include_empty {
                    out.push(Group { key: "a:none".into(), label: "UNASSIGNED".into(), color: t.text3, status: None, priority: None, assignee: None, due: None, tasks: ids });
                }
            }
            GroupBy::DueDate => {
                // Bucket rule from the original's due-date grouping (see MECHANICS.md):
                // closed/done -> "done", no date -> "no due date", any past day -> "overdue",
                // otherwise the day offset from today, capped at DUE_GROUP_CAP days ("future").
                let td = today();
                let mut keys: Vec<String> = vec!["-1".into()];
                for i in 0..DUE_GROUP_CAP {
                    keys.push(format!("{}", i));
                }
                keys.push(format!("+{}", DUE_GROUP_CAP));
                keys.push("null".into());
                keys.push("done".into());
                for k in keys {
                    if k == "done" && !self.show_closed && !include_empty {
                        continue;
                    }
                    let ids: Vec<u64> = tasks.iter().filter(|x| due_group_key(x.due, self.data.is_closed(x)) == k).map(|x| x.id).collect();
                    if ids.is_empty() && !include_empty {
                        continue;
                    }
                    let (label, due, color) = match k.as_str() {
                        "-1" => ("OVERDUE".to_string(), Some(td - Duration::days(1)), t.danger),
                        "0" => ("TODAY".to_string(), Some(td), Color32::from_rgb(0xe1, 0x6b, 0x16)),
                        "1" => ("TOMORROW".to_string(), Some(td + Duration::days(1)), rgb([0x10, 0x90, 0xe0])),
                        "null" => ("NO DUE DATE".to_string(), None, t.text3),
                        "done" => ("DONE".to_string(), None, rgb([0x00, 0x88, 0x44])),
                        _ if k.starts_with('+') => ("FUTURE".to_string(), Some(td + Duration::days(DUE_GROUP_CAP)), rgb([0x5f, 0x55, 0xee])),
                        _ => {
                            let n: i64 = k.parse().unwrap_or(2);
                            let d = td + Duration::days(n);
                            (d.format("%A").to_string().to_uppercase(), Some(d), rgb([0x44, 0x66, 0xff]))
                        }
                    };
                    out.push(Group { key: format!("d:{}", k), label, color, status: None, priority: None, assignee: None, due, tasks: ids });
                }
            }
            GroupBy::Tags => {
                // a task with several tags shows under each of them
                let mut tags: Vec<String> = tasks.iter().flat_map(|x| x.tags.iter().cloned()).collect();
                tags.sort_by_key(|g| g.to_lowercase());
                tags.dedup();
                for tag in tags {
                    let ids: Vec<u64> = tasks.iter().filter(|x| x.tags.contains(&tag)).map(|x| x.id).collect();
                    out.push(Group { key: format!("t:{}", tag), label: tag.to_uppercase(), color: tag_color(&tag), status: None, priority: None, assignee: None, due: None, tasks: ids });
                }
                let ids: Vec<u64> = tasks.iter().filter(|x| x.tags.is_empty()).map(|x| x.id).collect();
                if !ids.is_empty() || include_empty {
                    out.push(Group { key: "t:".into(), label: "NO TAGS".into(), color: t.text3, status: None, priority: None, assignee: None, due: None, tasks: ids });
                }
            }
            GroupBy::List => {
                let mut lists: Vec<u64> = self.data.lists_in(self.loc).unwrap_or_default();
                for x in tasks {
                    if !lists.contains(&x.list_id) {
                        lists.push(x.list_id);
                    }
                }
                for l in lists {
                    let ids: Vec<u64> = tasks.iter().filter(|x| x.list_id == l).map(|x| x.id).collect();
                    if ids.is_empty() && !include_empty {
                        continue;
                    }
                    let color = self.data.list_item(l).and_then(|li| li.color).map(rgb).unwrap_or(t.text2);
                    out.push(Group { key: format!("l:{}", l), label: self.data.list_name(l).to_uppercase(), color, status: None, priority: None, assignee: None, due: None, tasks: ids });
                }
            }
            GroupBy::None => {
                out.push(Group { key: "all".into(), label: "ALL TASKS".into(), color: t.primary, status: None, priority: None, assignee: None, due: None, tasks: tasks.iter().map(|x| x.id).collect() });
            }
        }
        out
    }

    /// The list new tasks go to from the current location.
    fn default_list(&self) -> Option<u64> {
        match self.loc {
            Location::List(id) => Some(id),
            _ => self.data.lists_in(self.loc).and_then(|v| v.first().copied()).or_else(|| self.data.first_list()),
        }
    }

    /// The list a group stands for (Group: List), or None.
    fn group_list(g: &Group) -> Option<u64> {
        g.key.strip_prefix("l:").and_then(|x| x.parse().ok())
    }

    /// The tag a group stands for (Group: Tags; "" for No tags), or None.
    fn group_tag(g: &Group) -> Option<&str> {
        g.key.strip_prefix("t:")
    }

    /// Gives a task the field value of the group it is dropped into.
    fn move_to_group(&self, task: &mut Task, g: &Group) {
        match self.group_by {
            GroupBy::Status => {
                if let Some(st) = &g.status {
                    if self.data.statuses_for_list(task.list_id).iter().any(|s| s.name == st.name) {
                        task.status = st.name.clone();
                    }
                }
            }
            GroupBy::Priority => task.priority = g.priority.unwrap_or(0),
            GroupBy::Assignee => task.assignees = g.assignee.into_iter().collect(),
            GroupBy::DueDate => task.due = g.due,
            GroupBy::Tags => match Self::group_tag(g) {
                Some("") => task.tags.clear(),
                Some(tag) if !task.tags.iter().any(|x| x == tag) => task.tags.push(tag.to_string()),
                _ => {}
            },
            GroupBy::List => {
                if let Some(l) = Self::group_list(g) {
                    task.list_id = l;
                }
            }
            GroupBy::None => {}
        }
    }

    fn add_from_group(&mut self, g: &Group, name: String, open: bool) {
        if let Some(list) = Self::group_list(g).or_else(|| self.default_list()) {
            if let Some(tag) = Self::group_tag(g).filter(|x| !x.is_empty()) {
                self.pending_tags.push(tag.to_string());
            }
            let mut assignees = vec![];
            if let Some(a) = g.assignee {
                assignees.push(a);
            }
            if self.loc == Location::MyTasks && !assignees.contains(&self.data.me) {
                assignees.push(self.data.me);
            }
            self.actions.push(Action::Add {
                list,
                name,
                status: g.status.as_ref().map(|s| s.name.clone()),
                priority: g.priority.unwrap_or(0),
                assignees,
                due: g.due,
                parent: None,
                open,
            });
        }
    }

    // ---------------------------------------------------------------- actions

    fn apply_actions(&mut self) {
        let actions = std::mem::take(&mut self.actions);
        if actions.iter().any(|a| matches!(a, Action::DeleteTask(_))) {
            // a new delete: the Undo bar offers this one only
            self.undo_trash = Some((vec![], now_ts()));
        }
        for a in actions {
            self.apply(a);
        }
    }

    fn log(task: &mut Task, me: u64, text: String) {
        task.activity.push(Activity { ts: now_ts(), author: me, text, comment: false, ..Default::default() });
    }

    /// Logs a change, or updates my own line about the same change if I made
    /// it in the last few minutes (`like` is part of that line).
    fn log_edit(task: &mut Task, me: u64, like: &str, text: String) {
        let now = now_ts();
        if let Some(last) = task.activity.last_mut() {
            if !last.comment && last.author == me && now - last.ts < 300 && last.text.contains(like) {
                last.text = text;
                last.ts = now;
                return;
            }
        }
        Self::log(task, me, text);
    }

    fn apply(&mut self, a: Action) {
        let me = self.data.me;
        match a {
            Action::Open(id) => {
                self.open_task = Some(id);
                self.recent.retain(|r| *r != id);
                self.recent.insert(0, id);
                self.recent.truncate(6);
                self.swap_comment_draft(id);
            }
            Action::Replace(mut new) => {
                if let Some(idx) = self.data.tasks.iter().position(|t| t.id == new.id) {
                    let old = self.data.tasks[idx].clone();
                    if old.status != new.status {
                        let m = format!("changed status from {} to {}", old.status.to_uppercase(), new.status.to_uppercase());
                        Self::log(&mut new, me, m);
                        // Date done / Date closed: when it entered such a status
                        let kind = |t: &Task| self.data.status_of(t).kind;
                        let (was, now_k) = (kind(&old), kind(&new));
                        let done = |k: StatusKind| matches!(k, StatusKind::Done | StatusKind::Closed);
                        if done(now_k) && !done(was) {
                            new.done_at = Some(now_ts());
                        } else if !done(now_k) {
                            new.done_at = None;
                        }
                        if now_k == StatusKind::Closed && was != StatusKind::Closed {
                            new.closed_at = Some(now_ts());
                        } else if now_k != StatusKind::Closed {
                            new.closed_at = None;
                        }
                    }
                    if old.priority != new.priority {
                        let m = format!("set priority to {}", PRIORITIES[new.priority as usize].label);
                        Self::log(&mut new, me, m);
                    }
                    if old.due != new.due {
                        let s = new.due.map(fmt_date).unwrap_or_else(|| "none".into());
                        Self::log(&mut new, me, format!("changed due date to {}", s));
                    }
                    if old.start != new.start {
                        let s = new.start.map(fmt_date).unwrap_or_else(|| "none".into());
                        Self::log(&mut new, me, format!("changed start date to {}", s));
                    }
                    for aid in new.assignees.clone().iter() {
                        if !old.assignees.contains(aid) {
                            let n = self.data.member(*aid).map(|m| m.name.clone()).unwrap_or_default();
                            Self::log(&mut new, me, format!("assigned to {}", n));
                        }
                    }
                    for aid in &old.assignees {
                        if !new.assignees.contains(aid) {
                            let n = self.data.member(*aid).map(|m| m.name.clone()).unwrap_or_default();
                            Self::log(&mut new, me, format!("removed assignee {}", n));
                        }
                    }
                    // typed fields change on every key press: while I keep typing,
                    // the line I logged a moment ago is updated instead of adding more
                    if old.name != new.name && !new.name.trim().is_empty() {
                        let m = format!("renamed this task to \"{}\"", new.name.trim());
                        Self::log_edit(&mut new, me, "renamed this task to ", m);
                    }
                    if old.description != new.description {
                        Self::log_edit(&mut new, me, "updated the description", "updated the description".into());
                    }
                    if old.estimate_min != new.estimate_min {
                        let m = match new.estimate_min {
                            Some(min) => format!("set the time estimate to {}", fmt_duration(min as i64 * 60)),
                            None => "removed the time estimate".into(),
                        };
                        Self::log_edit(&mut new, me, "time estimate", m);
                    }
                    for tag in new.tags.clone() {
                        if !old.tags.contains(&tag) {
                            Self::log(&mut new, me, format!("added tag {}", tag));
                        }
                    }
                    for tag in &old.tags {
                        if !new.tags.contains(tag) {
                            Self::log(&mut new, me, format!("removed tag {}", tag));
                        }
                    }
                    if new.name.trim().is_empty() {
                        new.name = old.name.clone();
                    }
                    if old.status != new.status && self.data.extra.labs.iter().any(|l| l == "confetti") {
                        let closes = self.data.statuses_for_list(new.list_id).iter().any(|s| s.name == new.status && matches!(s.kind, StatusKind::Closed | StatusKind::Done));
                        if closes {
                            self.toast = Some((format!("\u{1F389} Nice! \"{}\" is complete", new.name), now_ts()));
                        }
                    }
                    if old.status != new.status {
                        let rules: Vec<Automation> = self.data.extra.automations.iter().filter(|a| a.enabled && a.trigger_status == new.status).cloned().collect();
                        for a in rules {
                            match a.action.as_str() {
                                "assign" => {
                                    if let Ok(uid) = a.value.parse::<u64>() {
                                        if !new.assignees.contains(&uid) {
                                            new.assignees.push(uid);
                                        }
                                    }
                                }
                                "priority" => {
                                    if let Ok(p) = a.value.parse::<u8>() {
                                        new.priority = p.min(4);
                                    }
                                }
                                "move" => {
                                    if let Ok(l) = a.value.parse::<u64>() {
                                        if self.data.space_of_list(l).is_some() {
                                            new.list_id = l;
                                        }
                                    }
                                }
                                _ => new.activity.push(Activity { ts: now_ts(), author: me, text: a.value.clone(), comment: true, ..Default::default() }),
                            }
                            Self::log(&mut new, me, format!("Automation ran: {}", self.describe_action(&a)));
                        }
                    }
                    self.roll_recurring(&old, &mut new);
                    new.updated = now_ts();
                    self.data.tasks[idx] = new;
                    self.dirty = true;
                }
            }
            Action::Add { list, name, status, priority, assignees, due, parent, open } => {
                let id = self.data.new_id();
                let statuses = self.data.statuses_for_list(list);
                let status = status.unwrap_or_else(|| statuses[0].name.clone());
                let order = self.data.tasks.iter().map(|t| t.order).max().unwrap_or(0) + 1;
                let now = now_ts();
                self.data.tasks.push(Task {
                    id,
                    list_id: list,
                    parent,
                    name: if name.trim().is_empty() { "Untitled task".into() } else { name.trim().to_string() },
                    description: String::new(),
                    status,
                    priority,
                    assignees,
                    start: None,
                    due,
                    tags: vec![],
                    checklist: vec![],
                    activity: vec![Activity { ts: now, author: me, text: "created this task".into(), comment: false, ..Default::default() }],
                    time_entries: vec![],
                    timer_started: None,
                    estimate_min: None,
                    created: now,
                    updated: now,
                    order,
                    custom: Default::default(),
                    recur: None,
                    archived: false,
                    done_at: None,
                    closed_at: None,
                });
                if let Some(tp) = self.pending_template.take() {
                    if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == id) {
                        t.description = tp.description.clone();
                        t.tags = tp.tags.clone();
                        t.checklist = tp.checklist.iter().map(|c| CheckItem { text: c.clone(), done: false }).collect();
                    }
                }
                if let Some(desc) = self.pending_description.take() {
                    if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == id) {
                        t.description = desc;
                    }
                }
                let tags = std::mem::take(&mut self.pending_tags);
                if !tags.is_empty() {
                    if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == id) {
                        for tag in tags {
                            if !t.tags.contains(&tag) {
                                t.tags.push(tag);
                            }
                        }
                    }
                }
                self.dirty = true;
                if open {
                    self.open_task = Some(id);
                }
            }
            Action::DeleteTask(id) => {
                let mut kill = vec![id];
                let mut i = 0;
                while i < kill.len() {
                    let k = kill[i];
                    kill.extend(self.data.tasks.iter().filter(|t| t.parent == Some(k)).map(|t| t.id));
                    i += 1;
                }
                let removed: Vec<Task> = self.data.tasks.iter().filter(|t| kill.contains(&t.id)).cloned().collect();
                if let Some(first) = removed.iter().find(|t| t.id == id) {
                    let tid = self.data.new_id();
                    let title = first.name.clone();
                    self.data.extra.trash.push(TrashItem { id: tid, kind: "Task".into(), title, deleted: now_ts(), payload: serde_json::to_string(&removed).unwrap_or_default() });
                    if let Some((v, _)) = self.undo_trash.as_mut() {
                        v.push(tid);
                    }
                }
                self.data.tasks.retain(|t| !kill.contains(&t.id));
                if self.open_task.map(|o| kill.contains(&o)).unwrap_or(false) {
                    self.open_task = None;
                }
                self.dirty = true;
            }
            Action::Go(l) => {
                if !matches!(l, Location::Home | Location::Inbox | Location::Replies | Location::Assigned | Location::MyTasks | Location::TodayOverdue) && !self.data.location_exists(l) {
                    return;
                }
                self.section = Section::Home;
                if matches!(l, Location::Space(_)) && self.loc != l && !self.data.extra.hidden_views.iter().any(|h| h == "Overview") {
                    // spaces open on their Overview, as in the original
                    self.view = ViewKind::Overview;
                }
                self.loc = l;
                self.open_task = None;
                self.collapsed.clear();
            }
            Action::ToggleSpace(id) => {
                if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == id) {
                    s.expanded = !s.expanded;
                    self.dirty = true;
                }
            }
            Action::ToggleFolder(id) => {
                for s in &mut self.data.spaces {
                    for f in &mut s.folders {
                        if f.id == id {
                            f.expanded = !f.expanded;
                            self.dirty = true;
                        }
                    }
                }
            }
            Action::NewSpace => {
                let id = self.data.new_id();
                let lid = self.data.new_id();
                let color = STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13];
                self.data.spaces.push(Space { id, name: "New Space".into(), color, statuses: default_statuses(), folders: vec![], lists: vec![ListItem { id: lid, name: "List".into(), ..Default::default() }], expanded: true, private_owner: None, ..Default::default() });
                self.rename = Some((Target::Space(id), "New Space".into()));
                self.dirty = true;
            }
            Action::NewFolder(sid) => {
                let id = self.data.new_id();
                let lid = self.data.new_id();
                if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == sid) {
                    s.folders.push(Folder { id, name: "New Folder".into(), lists: vec![ListItem { id: lid, name: "List".into(), ..Default::default() }], expanded: true });
                    s.expanded = true;
                }
                self.rename = Some((Target::Folder(id), "New Folder".into()));
                self.dirty = true;
            }
            Action::NewList { space, folder } => {
                let id = self.data.new_id();
                if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == space) {
                    s.expanded = true;
                    let item = ListItem { id, name: "New List".into(), ..Default::default() };
                    match folder {
                        Some(fid) => {
                            if let Some(f) = s.folders.iter_mut().find(|f| f.id == fid) {
                                f.expanded = true;
                                f.lists.push(item);
                            }
                        }
                        None => s.lists.push(item),
                    }
                }
                self.loc = Location::List(id);
                self.rename = Some((Target::List(id), "New List".into()));
                self.dirty = true;
            }
            Action::StartRename(tg) => {
                let name = match tg {
                    Target::Space(id) => self.data.location_name(Location::Space(id)),
                    Target::Folder(id) => self.data.location_name(Location::Folder(id)),
                    Target::List(id) => self.data.location_name(Location::List(id)),
                    Target::Task(id) => self.task(id).map(|t| t.name.clone()).unwrap_or_default(),
                    Target::Workspace => self.data.workspace.clone(),
                };
                self.rename = Some((tg, name));
            }
            Action::AskDelete(tg) => self.confirm_delete = Some(tg),
            Action::ToggleTimer(id) => {
                let now = now_ts();
                let running = self.task(id).and_then(|t| t.timer_started).is_some();
                for t in &mut self.data.tasks {
                    if let Some(st) = t.timer_started.take() {
                        t.time_entries.push(TimeEntry { start: st, end: now });
                    }
                }
                if !running {
                    if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == id) {
                        t.timer_started = Some(now);
                    }
                }
                self.dirty = true;
            }
            Action::ToggleFavorite(l) => {
                if let Some(i) = self.data.favorites.iter().position(|f| *f == l) {
                    self.data.favorites.remove(i);
                } else {
                    self.data.favorites.push(l);
                }
                self.dirty = true;
            }
            Action::EditStatuses(sid) => self.edit_statuses = Some(sid),
            Action::SetSpaceColor(sid, c) => {
                if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == sid) {
                    s.color = c;
                    self.dirty = true;
                }
            }
        }
    }

    fn finish_rename(&mut self, tg: Target, name: String) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        match tg {
            Target::Space(id) => {
                if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == id) {
                    s.name = name;
                }
            }
            Target::Folder(id) => {
                for s in &mut self.data.spaces {
                    for f in &mut s.folders {
                        if f.id == id {
                            f.name = name.clone();
                        }
                    }
                }
            }
            Target::List(id) => {
                for s in &mut self.data.spaces {
                    for l in s.lists.iter_mut().chain(s.folders.iter_mut().flat_map(|f| f.lists.iter_mut())) {
                        if l.id == id {
                            l.name = name.clone();
                        }
                    }
                }
            }
            Target::Task(id) => {
                if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == id) {
                    t.name = name;
                }
            }
            Target::Workspace => self.data.workspace = name,
        }
        self.dirty = true;
    }

    fn do_delete(&mut self, tg: Target) {
        let mut lists: Vec<u64> = vec![];
        match tg {
            Target::Space(id) => {
                if let Some(l) = self.data.lists_in(Location::Space(id)) {
                    lists = l;
                }
                self.data.spaces.retain(|s| s.id != id);
            }
            Target::Folder(id) => {
                if let Some(l) = self.data.lists_in(Location::Folder(id)) {
                    lists = l;
                }
                for s in &mut self.data.spaces {
                    s.folders.retain(|f| f.id != id);
                }
            }
            Target::List(id) => {
                lists = vec![id];
                if let Some(sp) = self.data.space_of_list(id) {
                    let sid = sp.id;
                    let item = ListItem { id, name: self.data.list_name(id), ..Default::default() };
                    let tasks: Vec<Task> = self.data.tasks.iter().filter(|t| t.list_id == id).cloned().collect();
                    let tid = self.data.new_id();
                    self.data.extra.trash.push(TrashItem { id: tid, kind: "List".into(), title: item.name.clone(), deleted: now_ts(), payload: serde_json::to_string(&(sid, item, tasks)).unwrap_or_default() });
                }
                for s in &mut self.data.spaces {
                    s.lists.retain(|l| l.id != id);
                    for f in &mut s.folders {
                        f.lists.retain(|l| l.id != id);
                    }
                }
            }
            Target::Task(id) => {
                self.apply(Action::DeleteTask(id));
                return;
            }
            Target::Workspace => return,
        }
        self.data.tasks.retain(|t| !lists.contains(&t.list_id));
        let data = &self.data;
        self.data.favorites = data.favorites.iter().copied().filter(|f| data.location_exists(*f)).collect();
        if !self.data.location_exists(self.loc) {
            self.loc = Location::Home;
        }
        self.dirty = true;
    }

    // ---------------------------------------------------------------- top bar

    fn open_quick_add(&mut self, due: Option<NaiveDate>) {
        if let Some(list) = self.default_list() {
            self.quick_add = Some(QuickAdd { list, due, ..Default::default() });
        }
    }

    // ---------------------------------------------------------------- sidebar

    // ---------------------------------------------------------------- content

    fn content(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t.app_bg).inner_margin(egui::Margin { left: 0, right: 6, top: 0, bottom: 6 }))
            .show(ctx, |ui| {
                let card = ui.available_rect_before_wrap();
                ui.painter().rect_filled(card, 8.0, t.bg);
                let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(card.shrink(1.0)).layout(Layout::top_down(Align::Min)));
                inner.set_clip_rect(card.shrink(1.0));
                let ui = &mut inner;
                if !self.recent.is_empty() {
                    self.tray(ui);
                }
                let page = egui::Frame::new().fill(t.bg);
                match self.section {
                    Section::Home => {
                        self.sidebar(ui);
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| match self.loc {
                            Location::Home => self.home(ui),
                            Location::Inbox => self.inbox_page(ui),
                            Location::Replies => self.inbox(ui, 1),
                            Location::Assigned => self.inbox(ui, 2),
                            _ => {
                                self.header(ui);
                                let rect = ui.available_rect_before_wrap();
                                ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(16.0, 0.0))), |ui| match self.view {
                                    ViewKind::Overview => self.overview_view(ui),
                                    ViewKind::List => self.list_view(ui),
                                    ViewKind::Board => self.board_view(ui),
                                    ViewKind::Calendar => self.calendar_view(ui),
                                    ViewKind::Gantt => self.gantt_view(ui),
                                    ViewKind::Table => self.table_view(ui),
                                });
                            }
                        });
                    }
                    Section::Chat => {
                        self.sidebar(ui);
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.chat(ui));
                    }
                    Section::Planner => {
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.planner(ui));
                    }
                    Section::Posts => {
                        self.sidebar(ui);
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.posts_page(ui));
                    }
                    Section::Meetings => {
                        self.sidebar(ui);
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.meetings_page(ui));
                    }
                    Section::People => {
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.people_page(ui));
                    }
                    Section::Goals => {
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.goals_hub(ui));
                    }
                    Section::Forms => {
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.forms_hub(ui));
                    }
                    Section::Whiteboards => {
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.whiteboards_hub(ui));
                    }
                    Section::Settings => {
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.settings_page(ui));
                    }
                    Section::Ai => {
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.ai_page(ui));
                    }
                    Section::Teams => {
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.teams_hub(ui));
                    }
                    Section::Docs => {
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.docs_hub(ui));
                    }
                    Section::Dashboards => {
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.dashboards_hub(ui));
                    }
                    Section::Timesheets => {
                        egui::CentralPanel::default().frame(page).show_inside(ui, |ui| self.timesheets_hub(ui));
                    }
                }
                ui.painter().rect_stroke(card, 8.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
            });
    }

    // ---------------------------------------------------------------- list view

    // ---------------------------------------------------------------- board view

    // ---------------------------------------------------------------- calendar view

    fn calendar_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        let tasks: Vec<Task> = self.filtered().into_iter().cloned().collect();
        let td = today();
        // the week view keeps its own anchor (any day inside the shown week)
        let week_anchor: NaiveDate = ui.data(|d| d.get_temp(egui::Id::new("cal_week_anchor"))).unwrap_or(td);
        let mut week_anchor = week_anchor;
        ui.horizontal(|ui| {
            if ui.button("Today").clicked() {
                self.cal_month = NaiveDate::from_ymd_opt(td.year(), td.month(), 1).unwrap();
                week_anchor = td;
            }
            if ui.button("<").clicked() {
                if self.cal_week {
                    week_anchor -= Duration::days(7);
                } else {
                    self.cal_month = add_months(self.cal_month, -1);
                }
            }
            if ui.button(">").clicked() {
                if self.cal_week {
                    week_anchor += Duration::days(7);
                } else {
                    self.cal_month = add_months(self.cal_month, 1);
                }
            }
            let title = if self.cal_week {
                let ws = week_anchor - Duration::days(week_anchor.weekday().num_days_from_sunday() as i64);
                format!("{} \u{2013} {}", ws.format("%b %-d"), (ws + Duration::days(6)).format("%b %-d, %Y"))
            } else {
                self.cal_month.format("%B %Y").to_string()
            };
            ui.label(RichText::new(title).size(17.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.selectable_label(!self.cal_week, "Month").clicked() {
                    self.cal_week = false;
                    self.cal_month = NaiveDate::from_ymd_opt(week_anchor.year(), week_anchor.month(), 1).unwrap();
                }
                if ui.selectable_label(self.cal_week, "Week").clicked() {
                    self.cal_week = true;
                }
            });
        });
        ui.data_mut(|d| d.insert_temp(egui::Id::new("cal_week_anchor"), week_anchor));
        ui.add_space(6.0);
        let full = ui.available_rect_before_wrap();
        let side_w = 230.0;
        let avail = Rect::from_min_max(full.min, pos2(full.right() - side_w - 10.0, full.bottom()));
        let side = Rect::from_min_max(pos2(full.right() - side_w, full.top()), full.max);
        let cw = avail.width() / 7.0;
        let head = 26.0;
        let rows = if self.cal_week { 1 } else { 6 };
        let ch = if self.cal_week { (avail.height() - head - 10.0).max(200.0) } else { ((avail.height() - head - 34.0) / 6.0).max(96.0) };
        let area = Rect::from_min_size(avail.min, vec2(avail.width(), head + ch * rows as f32));
        ui.allocate_rect(full, Sense::hover());
        let p = ui.painter().clone();
        for (i, d) in ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].iter().enumerate() {
            p.text(pos2(area.left() + cw * i as f32 + 8.0, area.top() + head / 2.0), Align2::LEFT_CENTER, *d, FontId::proportional(12.0), t.text3);
        }
        let start = if self.cal_week {
            week_anchor - Duration::days(week_anchor.weekday().num_days_from_sunday() as i64)
        } else {
            self.cal_month - Duration::days(self.cal_month.weekday().num_days_from_sunday() as i64)
        };
        // a task dragged from the Unscheduled panel or another day
        let dragging = egui::DragAndDrop::has_payload_of_type::<u64>(ui.ctx());
        let released = ui.input(|i| i.pointer.any_released());
        let mut drop_on: Option<NaiveDate> = None;
        for i in 0..(rows * 7) as i64 {
            let d = start + Duration::days(i);
            let cell = Rect::from_min_size(pos2(area.left() + cw * (i % 7) as f32, area.top() + head + ch * (i / 7) as f32), vec2(cw, ch));
            let in_month = self.cal_week || d.month() == self.cal_month.month();
            if !in_month {
                p.rect_filled(cell, 0.0, t.column_bg);
            }
            let hov = ui.rect_contains_pointer(cell);
            if dragging && hov {
                p.rect_filled(cell, 0.0, t.selected_bg);
                if released {
                    drop_on = Some(d);
                }
            }
            p.rect_stroke(cell, 0.0, Stroke::new(0.5, t.border), egui::StrokeKind::Inside);
            let num_pos = cell.left_top() + vec2(10.0, 12.0);
            if d == td {
                p.circle_filled(num_pos + vec2(4.0, 0.0), 11.0, t.danger);
                p.text(num_pos + vec2(4.0, 0.0), Align2::CENTER_CENTER, d.day().to_string(), FontId::proportional(12.0), Color32::WHITE);
            } else {
                p.text(num_pos, Align2::LEFT_CENTER, d.day().to_string(), FontId::proportional(12.0), if in_month { t.text } else { t.text3 });
            }
            if hov && !dragging {
                let br = Rect::from_center_size(cell.right_top() + vec2(-14.0, 12.0), vec2(20.0, 20.0));
                let b = ui.interact(br, ui.id().with(("caladd", i)), Sense::click());
                p.rect_filled(br, 4.0, if b.hovered() { t.selected_bg } else { t.hover });
                p.text(br.center(), Align2::CENTER_CENTER, "+", FontId::proportional(14.0), t.text2);
                if b.clicked() {
                    self.open_quick_add(Some(d));
                }
            }
            let day_tasks: Vec<&Task> = tasks.iter().filter(|x| x.due == Some(d)).collect();
            let max = ((ch - 30.0) / 22.0).floor().max(1.0) as usize;
            for (k, task) in day_tasks.iter().enumerate() {
                if k >= max {
                    p.text(cell.left_top() + vec2(8.0, 28.0 + 22.0 * k as f32 + 9.0), Align2::LEFT_CENTER, format!("+{} more", day_tasks.len() - k), FontId::proportional(11.5), t.text3);
                    break;
                }
                let r = Rect::from_min_size(cell.left_top() + vec2(4.0, 26.0 + 22.0 * k as f32), vec2(cw - 8.0, 20.0));
                let resp = ui.interact(r, ui.id().with(("calt", task.id)), Sense::click_and_drag());
                if resp.drag_started() {
                    egui::DragAndDrop::set_payload(ui.ctx(), task.id);
                }
                let st = self.data.status_of(task);
                let col = rgb(st.color);
                p.rect_filled(r, 4.0, if resp.hovered() { col.gamma_multiply(0.30) } else { col.gamma_multiply(0.16) });
                p.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 2.0, col);
                let max_chars = ((cw - 24.0) / 6.5) as usize;
                p.with_clip_rect(r).text(r.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, truncate(&task.name, max_chars.max(4)), FontId::proportional(12.0), t.text);
                let tid = task.id;
                resp.context_menu(|ui| self.task_menu(ui, tid));
                if resp.clicked() {
                    self.actions.push(Action::Open(task.id));
                }
            }
        }
        // Unscheduled / Overdue panel
        p.rect_filled(side, 8.0, t.column_bg);
        let mut side_ui = ui.new_child(egui::UiBuilder::new().max_rect(side.shrink(10.0)).layout(Layout::top_down(Align::Min)));
        let tab: u8 = side_ui.data(|d| d.get_temp(egui::Id::new("cal_side_tab"))).unwrap_or(0);
        let unscheduled: Vec<&Task> = tasks.iter().filter(|x| x.due.is_none()).collect();
        let overdue: Vec<&Task> = tasks.iter().filter(|x| x.due.map_or(false, |d| d < td) && !self.data.is_closed(x)).collect();
        let mut new_tab = tab;
        side_ui.horizontal(|ui| {
            if ui.selectable_label(tab == 0, format!("{} Unscheduled", unscheduled.len())).clicked() {
                new_tab = 0;
            }
            if ui.selectable_label(tab == 1, format!("{} Overdue", overdue.len())).clicked() {
                new_tab = 1;
            }
        });
        side_ui.data_mut(|d| d.insert_temp(egui::Id::new("cal_side_tab"), new_tab));
        side_ui.label(RichText::new("Drag a task onto a day to schedule it.").small().color(t.text3));
        side_ui.add_space(4.0);
        let list = if tab == 0 { unscheduled } else { overdue };
        egui::ScrollArea::vertical().id_salt("cal_side").auto_shrink([false, false]).show(&mut side_ui, |ui| {
            for task in list {
                let w = ui.available_width();
                let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click_and_drag());
                if resp.drag_started() {
                    egui::DragAndDrop::set_payload(ui.ctx(), task.id);
                }
                ui.painter().rect_filled(r, 6.0, if resp.hovered() { t.hover } else { t.card });
                ui.painter().rect_stroke(r, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                let st = self.data.status_of(task);
                paint_status_circle(ui.painter(), r.left_center() + vec2(14.0, 0.0), 6.0, &st);
                ui.painter().with_clip_rect(r.shrink(2.0)).text(r.left_center() + vec2(28.0, 0.0), Align2::LEFT_CENTER, &task.name, FontId::proportional(12.5), t.text);
                if resp.clicked() {
                    self.actions.push(Action::Open(task.id));
                }
                ui.add_space(4.0);
            }
        });
        if dragging {
            if let Some(pos) = ui.ctx().pointer_hover_pos() {
                egui::Area::new(egui::Id::new("cal_drag_ghost")).order(egui::Order::Tooltip).fixed_pos(pos + vec2(10.0, 6.0)).show(ui.ctx(), |ui| {
                    ui.label(RichText::new("\u{1F4C5} Drop on a day").small());
                });
            }
        }
        if let Some(d) = drop_on {
            if let Some(id) = egui::DragAndDrop::take_payload::<u64>(ui.ctx()) {
                if let Some(mut task) = self.task(*id).cloned() {
                    task.due = Some(d);
                    if task.start.map_or(false, |s| s > d) {
                        task.start = Some(d);
                    }
                    self.actions.push(Action::Replace(task));
                }
            }
        } else if released && dragging {
            egui::DragAndDrop::clear_payload(ui.ctx());
        }
    }

    // ---------------------------------------------------------------- gantt view

    fn gantt_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        let tasks: Vec<Task> = self.filtered().into_iter().cloned().collect();
        let td = today();
        // scale: day columns get narrower as you zoom out
        let cw = match self.gantt_zoom {
            0 => 30.0,
            1 => 12.0,
            _ => 4.0,
        };
        let days: i64 = match self.gantt_zoom {
            0 => 70,
            1 => 182,
            _ => 548,
        };
        let back = match self.gantt_zoom {
            0 => 14,
            1 => 35,
            _ => 120,
        };
        let start = td - Duration::days(td.weekday().num_days_from_monday() as i64 + back);
        let rh = 34.0;
        let name_w = 260.0;
        let head = 44.0;
        ui.horizontal(|ui| {
            if ui.button("Today").clicked() {
                self.gantt_jump = true;
            }
            ui.separator();
            for (z, label) in [(0u8, "Day"), (1, "Week"), (2, "Month")] {
                if ui.selectable_label(self.gantt_zoom == z, label).clicked() && self.gantt_zoom != z {
                    self.gantt_zoom = z;
                    self.gantt_jump = true;
                }
            }
            if ui.small_button("\u{2212}").on_hover_text("Zoom out").clicked() && self.gantt_zoom < 2 {
                self.gantt_zoom += 1;
                self.gantt_jump = true;
            }
            if ui.small_button("+").on_hover_text("Zoom in").clicked() && self.gantt_zoom > 0 {
                self.gantt_zoom -= 1;
                self.gantt_jump = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Export CSV").on_hover_text("Save the tasks in this view as a CSV file").clicked() {
                    self.modal = Some(Modal::Import { path: String::new(), result: String::new() });
                }
                let undated = tasks.iter().filter(|x| x.start.is_none() && x.due.is_none()).count();
                if undated > 0 {
                    ui.label(RichText::new(format!("{} without dates", undated)).small().color(t.text3));
                }
            });
        });
        ui.add_space(4.0);
        // fill the view so the horizontal scrollbar sits at the bottom, not under the last row
        let rows_h = (head + rh * (tasks.len() + 1) as f32).max(ui.available_height() - 24.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                // names
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.set_width(name_w);
                    let (hr, _) = ui.allocate_exact_size(vec2(name_w, head), Sense::hover());
                    ui.painter().text(hr.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, "Name", FontId::proportional(12.0), t.text3);
                    ui.painter().hline(hr.x_range(), hr.bottom(), Stroke::new(1.0, t.border));
                    for task in &tasks {
                        let (r, resp) = ui.allocate_exact_size(vec2(name_w, rh), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 0.0, t.hover);
                        }
                        let st = self.data.status_of(task);
                        paint_status_circle(ui.painter(), r.left_center() + vec2(14.0, 0.0), 6.5, &st);
                        ui.painter().with_clip_rect(r.shrink(2.0)).text(r.left_center() + vec2(30.0, 0.0), Align2::LEFT_CENTER, &task.name, FontId::proportional(13.0), t.text);
                        ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(0.5, t.border));
                        let tid = task.id;
                        resp.context_menu(|ui| self.task_menu(ui, tid));
                        if resp.clicked() {
                            self.actions.push(Action::Open(task.id));
                        }
                    }
                    let (r, resp) = ui.allocate_exact_size(vec2(name_w, rh), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 0.0, t.hover);
                    }
                    ui.painter().text(r.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, "+ Add Task", FontId::proportional(13.0), t.text3);
                    if resp.clicked() {
                        self.open_quick_add(None);
                    }
                });
                // the divider only runs as far as the rows (it used to run to the bottom)
                let vr = ui.available_rect_before_wrap();
                ui.painter().vline(vr.left(), vr.top()..=vr.top() + rows_h, Stroke::new(1.0, t.border));
                let jump = std::mem::take(&mut self.gantt_jump);
                let mut sa = egui::ScrollArea::horizontal().id_salt(("gantt_h", self.gantt_zoom)).auto_shrink([false, true]);
                if jump {
                    let today_x = cw * (td - start).num_days() as f32;
                    sa = sa.horizontal_scroll_offset((today_x - vr.width() * 0.3).max(0.0));
                }
                sa.show(ui, |ui| {
                    let total_h = rows_h;
                    let (area, _) = ui.allocate_exact_size(vec2(cw * days as f32, total_h), Sense::hover());
                    let p = ui.painter().clone();
                    for i in 0..days {
                        let d = start + Duration::days(i);
                        let x = area.left() + cw * i as f32;
                        let col_r = Rect::from_min_size(pos2(x, area.top() + head), vec2(cw, total_h - head));
                        if self.gantt_zoom == 0 && d.weekday().num_days_from_monday() >= 5 {
                            p.rect_filled(col_r, 0.0, t.column_bg);
                        }
                        let tick = match self.gantt_zoom {
                            0 | 1 => d.weekday().num_days_from_monday() == 0,
                            _ => d.day() == 1,
                        };
                        if tick {
                            p.vline(x, area.top()..=area.top() + total_h, Stroke::new(0.5, t.border));
                            let label = if self.gantt_zoom == 2 { d.format("%b %Y").to_string() } else { d.format("%b %-d").to_string() };
                            p.text(pos2(x + 4.0, area.top() + 12.0), Align2::LEFT_CENTER, label, FontId::proportional(11.5), t.text2);
                        }
                        if self.gantt_zoom == 0 {
                            let dc = if d == td { Color32::WHITE } else { t.text3 };
                            if d == td {
                                p.circle_filled(pos2(x + cw / 2.0, area.top() + 31.0), 9.0, t.danger);
                            }
                            p.text(pos2(x + cw / 2.0, area.top() + 31.0), Align2::CENTER_CENTER, d.day().to_string(), FontId::proportional(11.0), dc);
                        }
                    }
                    p.hline(area.x_range(), area.top() + head, Stroke::new(1.0, t.border));
                    let today_x = area.left() + cw * (td - start).num_days() as f32 + cw / 2.0;
                    p.vline(today_x, area.top() + head..=area.top() + total_h, Stroke::new(1.5, t.danger));
                    for (k, task) in tasks.iter().enumerate() {
                        let y = area.top() + head + rh * k as f32;
                        p.hline(area.x_range(), y + rh, Stroke::new(0.5, t.border));
                        let (s, e) = match (task.start, task.due) {
                            (Some(s), Some(e)) => (s.min(e), e.max(s)),
                            (None, Some(e)) => (e, e),
                            (Some(s), None) => (s, s),
                            _ => continue,
                        };
                        let x0 = area.left() + cw * (s - start).num_days() as f32 + 1.0;
                        let x1 = area.left() + cw * ((e - start).num_days() + 1) as f32 - 1.0;
                        let mut r = Rect::from_min_max(pos2(x0, y + 7.0), pos2(x1.max(x0 + 8.0), y + rh - 7.0));
                        // drag a bar to move its dates
                        let resp = ui.interact(r, ui.id().with(("bar", task.id)), Sense::click_and_drag());
                        let acc_id = resp.id.with("acc");
                        if resp.dragged() {
                            let dx = resp.drag_delta().x;
                            ui.data_mut(|d| *d.get_temp_mut_or_default::<f32>(acc_id) += dx);
                        }
                        let acc: f32 = ui.data(|d| d.get_temp(acc_id)).unwrap_or(0.0);
                        if resp.drag_stopped() {
                            ui.data_mut(|d| d.remove::<f32>(acc_id));
                        }
                        let shift_days = (acc / cw).round() as i64;
                        if resp.dragged() {
                            r = r.translate(vec2(shift_days as f32 * cw, 0.0));
                        }
                        let col = rgb(self.data.status_of(task).color);
                        p.rect_filled(r, 5.0, if resp.hovered() || resp.dragged() { col } else { col.gamma_multiply(0.85) });
                        if r.width() > 60.0 {
                            p.with_clip_rect(r).text(r.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, &task.name, FontId::proportional(11.5), Color32::WHITE);
                        } else {
                            p.text(r.right_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, &task.name, FontId::proportional(11.5), t.text2);
                        }
                        if resp.drag_stopped() && shift_days != 0 {
                            let mut e2 = task.clone();
                            e2.start = e2.start.map(|d| d + Duration::days(shift_days));
                            e2.due = e2.due.map(|d| d + Duration::days(shift_days));
                            self.actions.push(Action::Replace(e2));
                        } else if resp.clicked() {
                            self.actions.push(Action::Open(task.id));
                        }
                    }
                });
            });
        });
    }

    // ---------------------------------------------------------------- table view

    fn table_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        let tasks: Vec<Task> = self.filtered().into_iter().cloned().collect();
        let now = now_ts();
        let widths = [44.0, 300.0, 130.0, 110.0, 110.0, 110.0, 150.0, 100.0, 110.0];
        let heads = ["#", "Name", "Status", "Assignee", "Due date", "Priority", "Tags", "Time tracked", "Date created"];
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let total: f32 = widths.iter().sum();
            let (hr, _) = ui.allocate_exact_size(vec2(total, 30.0), Sense::hover());
            let mut x = hr.left();
            for (i, h) in heads.iter().enumerate() {
                ui.painter().text(pos2(x + 8.0, hr.center().y), Align2::LEFT_CENTER, *h, FontId::proportional(12.0), t.text3);
                ui.painter().vline(x, hr.y_range(), Stroke::new(0.5, t.border));
                x += widths[i];
            }
            ui.painter().rect_filled(Rect::from_min_size(hr.left_bottom() - vec2(0.0, 1.0), vec2(total, 1.0)), 0.0, t.border);
            for (n, task) in tasks.iter().enumerate() {
                let (rr, resp) = ui.allocate_exact_size(vec2(total, 34.0), Sense::click());
                let id = ui.id().with(("trow", task.id));
                if self.selected.contains(&task.id) {
                    ui.painter().rect_filled(rr, 0.0, t.selected_bg);
                }
                if resp.hovered() {
                    ui.painter().rect_filled(rr, 0.0, t.hover);
                }
                ui.painter().hline(rr.x_range(), rr.bottom(), Stroke::new(0.5, t.border));
                let mut cells = vec![];
                let mut x = rr.left();
                for w in widths {
                    cells.push(Rect::from_min_size(pos2(x, rr.top()), vec2(w, rr.height())));
                    ui.painter().vline(x, rr.y_range(), Stroke::new(0.5, t.border));
                    x += w;
                }
                let st = self.data.status_of(task);
                let closed = matches!(st.kind, StatusKind::Closed | StatusKind::Done);
                let mut edited = task.clone();
                let mut changed = false;
                // row number, or the selection box on hover
                let on = self.selected.contains(&task.id);
                if resp.hovered() || on || !self.selected.is_empty() || ui.rect_contains_pointer(cells[0]) {
                    // row_checkbox centres the box 2px right of the rect's left edge
                    let cell = Rect::from_min_size(pos2(cells[0].center().x - 2.0, cells[0].top()), vec2(1.0, cells[0].height()));
                    self.row_checkbox(ui, cell, task.id, true);
                } else {
                    ui.painter().text(cells[0].center(), Align2::CENTER_CENTER, (n + 1).to_string(), FontId::proportional(12.0), t.text3);
                }
                let cells: Vec<Rect> = cells[1..].to_vec();
                ui.painter().with_clip_rect(cells[0].shrink(2.0)).text(cells[0].left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, &task.name, FontId::proportional(13.0), t.text);
                // status
                let sr = ui.interact(cells[1], id.with("s"), Sense::click());
                {
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(cells[1].shrink2(vec2(8.0, 6.0))).layout(Layout::left_to_right(Align::Center)));
                    status_pill(&mut child, &st, Sense::hover());
                }
                let statuses = self.data.statuses_for_list(task.list_id);
                if let Some(true) = popup(ui, &sr, id.with("sp"), 200.0, |ui| status_menu(ui, &t, &statuses, &mut edited.status)) {
                    changed = true;
                }
                // assignee
                let ar = ui.interact(cells[2], id.with("a"), Sense::click());
                paint_avatars(ui.painter(), cells[2].left_center() + vec2(8.0, 0.0), 10.0, &task.assignees, &self.data, t.bg);
                let data = &self.data;
                if let Some(true) = popup(ui, &ar, id.with("ap"), 220.0, |ui| assignee_menu(ui, &t, data, &mut edited.assignees)) {
                    changed = true;
                }
                // due
                let dr = ui.interact(cells[3], id.with("d"), Sense::click());
                if let Some(d) = task.due {
                    ui.painter().text(cells[3].left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, fmt_date(d), FontId::proportional(13.0), date_color(d, closed, &t));
                }
                if let Some(true) = popup(ui, &dr, id.with("dp"), 380.0, |ui| date_menu(ui, &t, id.with("dc"), &mut edited.due)) {
                    changed = true;
                }
                // priority
                let pr = ui.interact(cells[4], id.with("p"), Sense::click());
                if task.priority > 0 {
                    paint_flag(ui.painter(), cells[4].left_center() + vec2(10.0, 0.0), priority_color(task.priority, &t), 12.0);
                    ui.painter().text(cells[4].left_center() + vec2(24.0, 0.0), Align2::LEFT_CENTER, PRIORITIES[task.priority as usize].label, FontId::proportional(13.0), t.text);
                }
                if let Some(true) = popup(ui, &pr, id.with("pp"), 180.0, |ui| priority_menu(ui, &t, &mut edited.priority)) {
                    changed = true;
                }
                // tags
                {
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(cells[5].shrink2(vec2(8.0, 6.0))).layout(Layout::left_to_right(Align::Center)));
                    child.set_clip_rect(cells[5]);
                    for tag in &task.tags {
                        chip(&mut child, tag, tag_color(tag));
                    }
                }
                let secs = task.tracked_secs(now);
                if secs > 0 {
                    ui.painter().text(cells[6].left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, fmt_duration(secs), FontId::proportional(13.0), t.text2);
                }
                let created = chrono::DateTime::from_timestamp(task.created, 0).map(|d| d.with_timezone(&chrono::Local).date_naive()).unwrap_or(td_default());
                ui.painter().text(cells[7].left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, fmt_date(created), FontId::proportional(13.0), t.text2);
                let tid = task.id;
                resp.context_menu(|ui| self.task_menu(ui, tid));
                let in_box = resp.interact_pointer_pos().map_or(false, |p| p.x < rr.left() + 44.0);
                if changed {
                    self.actions.push(Action::Replace(edited));
                } else if resp.clicked() && !in_box {
                    if ui.input(|i| i.modifiers.command || i.modifiers.shift) {
                        if !self.selected.remove(&tid) {
                            self.selected.insert(tid);
                        }
                    } else {
                        self.actions.push(Action::Open(task.id));
                    }
                }
            }
            let (r, resp) = ui.allocate_exact_size(vec2(total, 32.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, 0.0, t.hover);
            }
            ui.painter().text(r.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, "+ Add Task", FontId::proportional(13.0), t.text3);
            if resp.clicked() {
                self.open_quick_add(None);
            }
        });
    }

    // ---------------------------------------------------------------- home & inbox

    fn home(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(32, 24)).show(ui, |ui| {
                let hour = chrono::Local::now().format("%H").to_string().parse::<u32>().unwrap_or(9);
                let greet = if hour < 12 { "Good morning" } else if hour < 18 { "Good afternoon" } else { "Good evening" };
                let me = self.data.member(self.data.me).map(|m| m.name.clone()).unwrap_or_default();
                ui.label(RichText::new(format!("{}, {}", greet, me)).size(24.0).strong());
                ui.label(RichText::new(chrono::Local::now().format("%A, %B %-d").to_string()).color(t.text2));
                ui.add_space(18.0);
                let td = today();
                let mine: Vec<Task> = self.data.tasks.iter().filter(|x| x.assignees.contains(&self.data.me) && !self.data.is_closed(x)).cloned().collect();
                let done_mine: Vec<Task> = self.data.tasks.iter().filter(|x| x.assignees.contains(&self.data.me) && self.data.is_closed(x)).cloned().collect();
                // Section order follows the original's My Work tabs: Today, Overdue, Next, Unscheduled, Done.
                let sections: [(&'static str, Vec<&Task>); 5] = [
                    ("Today", mine.iter().filter(|x| x.due == Some(td)).collect()),
                    ("Overdue", mine.iter().filter(|x| x.due.map(|d| d < td).unwrap_or(false)).collect()),
                    ("Next", mine.iter().filter(|x| x.due.map(|d| d > td).unwrap_or(false)).collect()),
                    ("Unscheduled", mine.iter().filter(|x| x.due.is_none()).collect()),
                    ("Done", done_mine.iter().collect()),
                ];
                let avail = ui.available_width();
                let card_w = ((avail - 20.0) / 2.0).max(320.0);
                ui.horizontal_top(|ui| {
                    // My Work card
                    egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::same(16)).show(ui, |ui| ui.vertical(|ui| {
                        ui.set_width(card_w - 34.0);
                        ui.label(RichText::new("My Work").size(16.0).strong());
                        ui.add_space(6.0);
                        for (name, list) in &sections {
                            let collapsed = self.home_collapsed.contains(name);
                            let (r, resp) = ui.allocate_exact_size(vec2(card_w - 34.0, 28.0), Sense::click());
                            paint_triangle(ui.painter(), r.left_center() + vec2(6.0, 0.0), !collapsed, t.text2);
                            let col = if *name == "Overdue" && !list.is_empty() { t.danger } else { t.text };
                            ui.painter().text(r.left_center() + vec2(18.0, 0.0), Align2::LEFT_CENTER, *name, FontId::proportional(13.5), col);
                            ui.painter().text(r.left_center() + vec2(110.0, 0.0), Align2::LEFT_CENTER, list.len().to_string(), FontId::proportional(12.0), t.text3);
                            if resp.clicked() {
                                if collapsed {
                                    self.home_collapsed.remove(name);
                                } else {
                                    self.home_collapsed.insert(name);
                                }
                            }
                            if !collapsed {
                                for task in list {
                                    let (r, resp) = ui.allocate_exact_size(vec2(card_w - 34.0, 30.0), Sense::click());
                                    if resp.hovered() {
                                        ui.painter().rect_filled(r, 6.0, t.hover);
                                    }
                                    let st = self.data.status_of(task);
                                    paint_status_circle(ui.painter(), r.left_center() + vec2(24.0, 0.0), 6.5, &st);
                                    ui.painter().with_clip_rect(Rect::from_min_max(r.min, pos2(r.right() - 140.0, r.bottom()))).text(r.left_center() + vec2(40.0, 0.0), Align2::LEFT_CENTER, &task.name, FontId::proportional(13.0), t.text);
                                    ui.painter().text(r.right_center() - vec2(70.0, 0.0), Align2::RIGHT_CENTER, truncate(&self.data.list_name(task.list_id), 14), FontId::proportional(11.5), t.text3);
                                    if let Some(d) = task.due {
                                        ui.painter().text(r.right_center() - vec2(6.0, 0.0), Align2::RIGHT_CENTER, fmt_date(d), FontId::proportional(12.0), date_color(d, false, &t));
                                    }
                                    if resp.clicked() {
                                        self.actions.push(Action::Open(task.id));
                                    }
                                }
                            }
                        }
                    }));
                    ui.add_space(12.0);
                    // Recents / lists overview
                    egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::same(16)).show(ui, |ui| ui.vertical(|ui| {
                        ui.set_width(card_w - 34.0);
                        ui.label(RichText::new("Lists").size(16.0).strong());
                        ui.add_space(6.0);
                        let spaces = self.data.spaces.clone();
                        for s in &spaces {
                            if s.archived {
                                continue;
                            }
                            let lists: Vec<ListItem> = s.lists.iter().cloned().chain(s.folders.iter().flat_map(|f| f.lists.iter().cloned())).filter(|l| !l.archived).collect();
                            for l in lists {
                                let total = self.data.tasks.iter().filter(|x| x.list_id == l.id && x.parent.is_none()).count();
                                let done = self.data.tasks.iter().filter(|x| x.list_id == l.id && x.parent.is_none() && self.data.is_closed(x)).count();
                                let (r, resp) = ui.allocate_exact_size(vec2(card_w - 34.0, 34.0), Sense::click());
                                if resp.hovered() {
                                    ui.painter().rect_filled(r, 6.0, t.hover);
                                }
                                ui.painter().rect_filled(Rect::from_center_size(r.left_center() + vec2(14.0, 0.0), vec2(14.0, 14.0)), 4.0, rgb(s.color));
                                ui.painter().text(r.left_center() + vec2(30.0, 0.0), Align2::LEFT_CENTER, &l.name, FontId::proportional(13.0), t.text);
                                // progress bar
                                let bar = Rect::from_min_size(pos2(r.right() - 150.0, r.center().y - 3.0), vec2(100.0, 6.0));
                                ui.painter().rect_filled(bar, 3.0, t.hover);
                                if total > 0 {
                                    let f = done as f32 / total as f32;
                                    ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(100.0 * f, 6.0)), 3.0, rgb([0x00, 0x88, 0x44]));
                                }
                                ui.painter().text(r.right_center() - vec2(6.0, 0.0), Align2::RIGHT_CENTER, format!("{}/{}", done, total), FontId::proportional(11.5), t.text3);
                                if resp.clicked() {
                                    self.actions.push(Action::Go(Location::List(l.id)));
                                }
                            }
                        }
                    }));
                });
            });
        });
    }

    fn inbox(&mut self, ui: &mut Ui, mode: u8) {
        let t = self.t();
        egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
            let (title, sub) = match mode {
                1 => ("Replies", "Comments from others on tasks and chat replies"),
                2 => ("Assigned Comments", "Comments assigned to someone, or that @mention them, until resolved"),
                _ => ("Inbox", "Recent activity across your workspace"),
            };
            ui.label(RichText::new(title).size(20.0).strong());
            ui.label(RichText::new(sub).color(t.text2));
            ui.add_space(8.0);
            // tabs: Unread / Read for Replies, Assigned to me / Delegated by me for comments
            let tab_id = egui::Id::new(("inbox_tab", mode));
            let mut tab: u8 = ui.data(|d| d.get_temp(tab_id)).unwrap_or(0);
            if mode != 0 {
                ui.horizontal(|ui| {
                    let labels = if mode == 1 { ["Unread", "Read"] } else { ["Assigned to me", "Delegated by me"] };
                    for (i, l) in labels.iter().enumerate() {
                        if ui.selectable_label(tab == i as u8, *l).clicked() {
                            tab = i as u8;
                        }
                    }
                    if mode == 1 && tab == 0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Mark all as read").clicked() {
                                self.data.extra.replies_seen = now_ts();
                                self.dirty = true;
                            }
                        });
                    }
                });
                ui.data_mut(|d| d.insert_temp(tab_id, tab));
            }
            let seen = self.data.extra.replies_seen;
            if mode == 0 {
                self.reminders_block(ui);
                if ui.button("\u{23F0}  Create Reminder").clicked() {
                    self.modal = Some(Modal::Reminder { text: String::new(), date: Some(today()), time: "09:00".into() });
                }
            }
            ui.add_space(10.0);
            let mut items: Vec<(i64, u64, String, Activity)> = vec![];
            let me = self.data.me;
            let my_name = self.data.member(me).map(|m| format!("@{}", m.name)).unwrap_or_default();
            for task in &self.data.tasks {
                for a in &task.activity {
                    let keep = match (mode, tab) {
                        (1, 0) => a.comment && a.author != me && a.ts > seen,
                        (1, _) => a.comment && a.author != me && a.ts <= seen,
                        // assigned to me, or (when assigned to no one) mentioning me
                        (2, 0) => a.comment && !a.resolved && (a.assignee == Some(me) || (a.assignee.is_none() && !my_name.is_empty() && a.text.contains(&my_name))),
                        (2, _) => a.comment && !a.resolved && a.author == me && (a.assignee.is_some_and(|x| x != me) || (a.assignee.is_none() && a.text.contains('@'))),
                        _ => true,
                    };
                    if keep {
                        items.push((a.ts, task.id, task.name.clone(), a.clone()));
                    }
                }
            }
            items.sort_by_key(|x| std::cmp::Reverse(x.0));
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if items.is_empty() {
                    ui.label(RichText::new("You're all caught up").color(t.text3));
                }
                for (ts, tid, tname, a) in items.into_iter().take(200) {
                    let w = ui.available_width();
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 56.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 8.0, t.hover);
                    }
                    ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(0.5, t.border));
                    if let Some(m) = self.data.member(a.author) {
                        paint_avatar(ui.painter(), r.left_center() + vec2(22.0, 0.0), 13.0, m, t.bg);
                    }
                    ui.painter().text(r.left_top() + vec2(48.0, 16.0), Align2::LEFT_CENTER, &tname, FontId::proportional(13.5), t.text);
                    let who = self.data.member(a.author).map(|m| m.name.clone()).unwrap_or_default();
                    let line = if a.comment { format!("{} commented: {}", who, a.text) } else { format!("{} {}", who, a.text) };
                    ui.painter().with_clip_rect(Rect::from_min_max(r.min, pos2(r.right() - 140.0, r.bottom()))).text(r.left_top() + vec2(48.0, 38.0), Align2::LEFT_CENTER, line, FontId::proportional(12.5), t.text2);
                    ui.painter().text(r.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, fmt_ago(ts), FontId::proportional(11.5), t.text3);
                    // tick off an assigned comment right here
                    let mut resolved = false;
                    if mode == 2 {
                        let br = Rect::from_center_size(r.right_center() - vec2(118.0, 0.0), vec2(26.0, 26.0));
                        let b = put_over(ui, br, egui::Button::new("\u{2610}").frame(false));
                        name_it(&b, &format!("Resolve comment on {}", tname));
                        if b.on_hover_text("Resolve").clicked() {
                            if let Some(c) = self.data.tasks.iter_mut().find(|x| x.id == tid).and_then(|x| x.activity.iter_mut().find(|c| c.id == a.id && a.id != 0)) {
                                c.resolved = true;
                                self.dirty = true;
                                resolved = true;
                            }
                        }
                    }
                    if resp.clicked() && !resolved {
                        self.actions.push(Action::Open(tid));
                    }
                }
            });
        });
    }

    // ---------------------------------------------------------------- task detail

    fn task_window(&mut self, ctx: &egui::Context) {
        let Some(tid) = self.open_task else { return };
        self.swap_comment_draft(tid);
        // comments need ids to be edited, reacted to and replied to
        if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == tid) {
            fix_task_activity_ids(t);
        }
        let Some(orig) = self.task(tid).cloned() else {
            self.open_task = None;
            return;
        };
        let t = self.t();
        let screen = ctx.screen_rect();
        // dim backdrop that closes on click
        let mut close = false;
        egui::Area::new(egui::Id::new("task_dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
            let r = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(screen, 0.0, t.overlay);
            if r.clicked() {
                close = true;
            }
        });
        let size = vec2(screen.width() - 48.0, screen.height() - 48.0);
        let win_id = egui::Id::new("task_window");
        let mut task = orig.clone();
        let now = now_ts();
        let statuses = self.data.statuses_for_list(task.list_id);
        egui::Window::new("task")
            .id(win_id)
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .fixed_size(size)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(egui::Frame::new().fill(t.card).corner_radius(12).stroke(Stroke::new(1.0, t.border)))
            .show(ctx, |ui| {
                ui.set_min_size(size);
                // header
                egui::Frame::new().inner_margin(egui::Margin::symmetric(18, 10)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // previous / next task in the current view
                        let (prev, next) = self.neighbours(tid);
                        if ui.add_enabled(prev.is_some(), egui::Button::new("\u{2191}").frame(false)).on_hover_text("Previous task").clicked() {
                            if let Some(p) = prev {
                                self.actions.push(Action::Open(p));
                            }
                        }
                        if ui.add_enabled(next.is_some(), egui::Button::new("\u{2193}").frame(false)).on_hover_text("Next task").clicked() {
                            if let Some(n) = next {
                                self.actions.push(Action::Open(n));
                            }
                        }
                        let path = self.data.list_path(task.list_id).join("  /  ");
                        if ui.add(egui::Button::new(RichText::new(path).color(t.text2)).frame(false)).on_hover_text("Move to another List").clicked() {
                            self.modal = Some(Modal::MoveTasks { ids: vec![tid], query: String::new() });
                        }
                        if task.archived {
                            ui.label(RichText::new("Archived").small().color(t.danger));
                        }
                        if let Some(pid) = task.parent {
                            if let Some(p) = self.task(pid) {
                                ui.label(RichText::new("/").color(t.text3));
                                if ui.link(truncate(&p.name, 30)).clicked() {
                                    self.actions.push(Action::Open(pid));
                                }
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("\u{2716}").size(14.0)).frame(false)).on_hover_text("Close").clicked() {
                                close = true;
                            }
                            let r = ui.add(egui::Button::new(RichText::new("\u{2026}").size(16.0)).frame(false));
                            if ui.add(egui::Button::new(RichText::new("ID").size(12.0)).frame(false)).on_hover_text("Copy task ID").clicked() {
                                ui.ctx().copy_text(tid.to_string());
                                self.toast = Some(("Task ID copied".into(), now_ts()));
                            }
                            popup(ui, &r, win_id.with("more"), 200.0, |ui| {
                                if ui.button("Move to...").clicked() {
                                    self.modal = Some(Modal::MoveTasks { ids: vec![tid], query: String::new() });
                                    close_popup(ui);
                                }
                                if ui.button("Duplicate").clicked() {
                                    if let Some(n) = self.duplicate_task(tid) {
                                        self.actions.push(Action::Open(n));
                                    }
                                    close_popup(ui);
                                }
                                if ui.button("Remind me").clicked() {
                                    self.modal = Some(Modal::Reminder { text: task.name.clone(), date: Some(today() + Duration::days(1)), time: "09:00".into() });
                                    close_popup(ui);
                                }
                                if ui.button("Copy ID").clicked() {
                                    ui.ctx().copy_text(tid.to_string());
                                    self.toast = Some(("Task ID copied".into(), now_ts()));
                                    close_popup(ui);
                                }
                                if ui.button(if task.archived { "Unarchive" } else { "Archive" }).clicked() {
                                    let a = !task.archived;
                                    self.set_archived(&[tid], a);
                                    close_popup(ui);
                                }
                                ui.separator();
                                if ui.button("Save as template").clicked() {
                                    let id = self.data.new_id();
                                    self.data.extra.templates.push(TaskTemplate { id, name: task.name.clone(), task_name: task.name.clone(), description: task.description.clone(), checklist: task.checklist.iter().map(|c| c.text.clone()).collect(), tags: task.tags.clone(), priority: task.priority });
                                    self.toast = Some(("Saved as a template".into(), now_ts()));
                                    self.dirty = true;
                                    close_popup(ui);
                                }
                                if ui.button("Copy link").clicked() {
                                    ui.ctx().copy_text(format!("workboard://task/{}", tid));
                                    self.toast = Some(("Link copied".into(), now_ts()));
                                    close_popup(ui);
                                }
                                if ui.button("Start timer").clicked() {
                                    self.actions.push(Action::ToggleTimer(tid));
                                    close_popup(ui);
                                }
                                if ui.button(RichText::new("Delete task").color(t.danger)).clicked() {
                                    self.actions.push(Action::DeleteTask(tid));
                                    close_popup(ui);
                                }
                            });
                        });
                    });
                });
                let r = ui.available_rect_before_wrap();
                ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
                let body = ui.available_rect_before_wrap();
                let right_w = 380.0;
                let left = Rect::from_min_max(body.min, pos2(body.right() - right_w, body.bottom()));
                let right = Rect::from_min_max(pos2(body.right() - right_w, body.top()), body.max);
                ui.painter().rect_filled(right, egui::CornerRadius { nw: 0, ne: 0, sw: 0, se: 12 }, t.column_bg);
                ui.painter().vline(right.left(), right.y_range(), Stroke::new(1.0, t.border));
                // ---- left
                let mut left_ui = ui.new_child(egui::UiBuilder::new().max_rect(left.shrink2(vec2(28.0, 14.0))).layout(Layout::top_down(Align::Min)));
                egui::ScrollArea::vertical().id_salt("detail_left").auto_shrink([false, false]).show(&mut left_ui, |ui| {
                    // task type label above the title, as in the original
                    let g = ui.painter().layout_no_wrap("Task".into(), FontId::proportional(13.5), t.text);
                    let (tr, _) = ui.allocate_exact_size(vec2(g.size().x + 44.0, 26.0), Sense::hover());
                    ui.painter().rect_stroke(tr, 7.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                    ui.painter().circle_filled(tr.left_center() + vec2(13.0, 0.0), 5.0, t.text2);
                    ui.painter().galley(pos2(tr.left() + 24.0, tr.center().y - g.size().y / 2.0), g, t.text);
                    paint_triangle(ui.painter(), tr.right_center() - vec2(10.0, 0.0), true, t.text2);
                    ui.add_space(6.0);
                    ui.add(egui::TextEdit::singleline(&mut task.name).font(FontId::proportional(30.0)).frame(false).desired_width(f32::INFINITY));
                    ui.add_space(10.0);
                    // a narrow window stacks the fields one per row instead of
                    // two side by side (they ran under the comments column)
                    let one_col = ui.available_width() < 560.0;
                    let field_w = if one_col { (ui.available_width() - 10.0).max(200.0) } else { (ui.available_width() / 2.0 - 10.0).max(200.0) };
                    let apps = self.data.extra.apps.clone();
                    let fields = self.data.extra.fields.clone();
                    egui::Grid::new("fields").num_columns(4).spacing(vec2(12.0, 10.0)).min_col_width(90.0).show(ui, |ui| {
                        // Status
                        ui.label(RichText::new("\u{25CE}  Status").color(t.text2));
                        ui.horizontal(|ui| {
                            ui.set_width(field_w - 110.0);
                            let st = self.data.status_of(&task);
                            let r = status_pill(ui, &st, Sense::click());
                            popup(ui, &r, win_id.with("st"), 200.0, |ui| status_menu(ui, &t, &statuses, &mut task.status));
                            // complete shortcut
                            if let Some(done) = statuses.iter().find(|s| s.kind == StatusKind::Closed) {
                                let is_done = task.status == done.name;
                                let txt = if is_done { "\u{2714} Done" } else { "\u{2714}" };
                                let b = egui::Button::new(RichText::new(txt).color(if is_done { Color32::WHITE } else { t.text2 }))
                                    .fill(if is_done { rgb(done.color) } else { Color32::TRANSPARENT })
                                    .stroke(Stroke::new(1.0, t.border));
                                if ui.add(b).on_hover_text("Mark complete").clicked() {
                                    task.status = if is_done { statuses[0].name.clone() } else { done.name.clone() };
                                }
                            }
                        });
                        if one_col {
                            ui.end_row();
                        }
                        // Assignees
                        ui.label(RichText::new("\u{1F464}  Assignees").color(t.text2));
                        let (ar, aresp) = ui.allocate_exact_size(vec2(field_w - 110.0, 26.0), Sense::click());
                        if aresp.hovered() {
                            ui.painter().rect_filled(ar, 6.0, t.hover);
                        }
                        let used = paint_avatars(ui.painter(), ar.left_center() + vec2(4.0, 0.0), 11.0, &task.assignees, &self.data, t.card);
                        if used == 0.0 {
                            ui.painter().text(ar.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, "Empty", FontId::proportional(13.0), t.text3);
                        }
                        let data = &self.data;
                        popup(ui, &aresp, win_id.with("as"), 220.0, |ui| assignee_menu(ui, &t, data, &mut task.assignees));
                        ui.end_row();
                        // Dates
                        ui.label(RichText::new("\u{1F4C5}  Dates").color(t.text2));
                        ui.horizontal(|ui| {
                            let s_txt = task.start.map(fmt_date).unwrap_or_else(|| "Start".into());
                            let r = ui.add(egui::Button::new(RichText::new(s_txt).color(if task.start.is_some() { t.text } else { t.text3 })).frame(false));
                            if r.clicked() {
                                ui.data_mut(|d| d.insert_temp(win_id.with("dates").with("which"), 0u8));
                            }
                            popup(ui, &r, win_id.with("sd"), 380.0, |ui| dates_menu(ui, &t, win_id.with("dates"), &mut task.start, &mut task.due, &mut task.recur));
                            ui.label(RichText::new("\u{2192}").color(t.text3));
                            let closed = statuses.iter().find(|s| s.name == task.status).map(|s| matches!(s.kind, StatusKind::Closed | StatusKind::Done)).unwrap_or(false);
                            let (d_txt, d_col) = match task.due {
                                Some(d) => (fmt_date(d), date_color(d, closed, &t)),
                                None => ("Due".into(), t.text3),
                            };
                            let r = ui.add(egui::Button::new(RichText::new(d_txt).color(d_col)).frame(false));
                            popup(ui, &r, win_id.with("dd"), 380.0, |ui| dates_menu(ui, &t, win_id.with("dates2"), &mut task.start, &mut task.due, &mut task.recur));
                            if task.recur.is_some() {
                                ui.label(RichText::new(format!("\u{1F501} {}", recur_label(&task.recur))).small().color(t.text2)).on_hover_text("Repeats when closed");
                            }
                        });
                        if one_col {
                            ui.end_row();
                        }
                        // Priority
                        if !apps.priorities {
                            if !one_col {
                                ui.label("");
                                ui.label("");
                            }
                        } else {
                        ui.label(RichText::new("\u{2691}  Priority").color(t.text2));
                        let (pr, presp) = ui.allocate_exact_size(vec2(field_w - 110.0, 26.0), Sense::click());
                        if presp.hovered() {
                            ui.painter().rect_filled(pr, 6.0, t.hover);
                        }
                        if task.priority > 0 {
                            paint_flag(ui.painter(), pr.left_center() + vec2(8.0, 0.0), priority_color(task.priority, &t), 13.0);
                            ui.painter().text(pr.left_center() + vec2(22.0, 0.0), Align2::LEFT_CENTER, PRIORITIES[task.priority as usize].label, FontId::proportional(13.0), t.text);
                        } else {
                            ui.painter().text(pr.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, "Empty", FontId::proportional(13.0), t.text3);
                        }
                        popup(ui, &presp, win_id.with("pr"), 180.0, |ui| priority_menu(ui, &t, &mut task.priority));
                        }
                        if !one_col || apps.priorities {
                            ui.end_row();
                        }
                        // Time estimate
                        if apps.time_estimates {
                        ui.label(RichText::new("\u{231B}  Time estimate").color(t.text2));
                        ui.horizontal(|ui| {
                            let mut est = task.estimate_min.unwrap_or(0);
                            if est == 0 {
                                ui.label(RichText::new("Empty").color(t.text3));
                            }
                            let r = ui.add(egui::DragValue::new(&mut est).range(0..=100000).speed(5).suffix(" min"));
                            if r.changed() {
                                task.estimate_min = if est == 0 { None } else { Some(est) };
                            }
                        });
                        } else if !one_col {
                            ui.label("");
                            ui.label("");
                        }
                        if one_col && apps.time_estimates {
                            ui.end_row();
                        }
                        // Track time
                        if !apps.time_tracking {
                            if !one_col {
                                ui.label("");
                                ui.label("");
                            }
                        } else {
                        ui.label(RichText::new("\u{23F1}  Track time").color(t.text2));
                        ui.horizontal(|ui| {
                            let running = task.timer_started.is_some();
                            let (txt, fill) = if running { ("\u{23F9}", t.danger) } else { ("\u{25B6}", t.primary) };
                            if ui.add(egui::Button::new(RichText::new(txt).color(Color32::WHITE)).fill(fill).corner_radius(12).min_size(vec2(24.0, 24.0))).on_hover_text(if running { "Stop timer" } else { "Start timer" }).clicked() {
                                self.actions.push(Action::ToggleTimer(tid));
                            }
                            let secs = task.tracked_secs(now);
                            ui.label(if secs > 0 { fmt_duration(secs) } else { "Start".into() });
                            if running {
                                ctx.request_repaint_after(std::time::Duration::from_secs(1));
                            }
                        });
                        }
                        if !one_col || apps.time_tracking {
                            ui.end_row();
                        }
                        // Tags
                        ui.label(RichText::new("\u{1F3F7}  Tags").color(t.text2));
                        ui.horizontal_wrapped(|ui| {
                            ui.set_max_width(field_w * 2.0);
                            let mut remove = None;
                            for (i, tag) in task.tags.iter().enumerate() {
                                if chip(ui, &format!("{}  \u{2716}", tag), tag_color(tag)).on_hover_text("Remove tag").clicked() {
                                    remove = Some(i);
                                }
                            }
                            if let Some(i) = remove {
                                task.tags.remove(i);
                            }
                            let r = ui.add(egui::TextEdit::singleline(&mut self.tag_text).hint_text("Add tag").desired_width(100.0));
                            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let tag = self.tag_text.trim().to_lowercase();
                                if !tag.is_empty() && !task.tags.contains(&tag) {
                                    task.tags.push(tag);
                                }
                                self.tag_text.clear();
                            }
                        });
                        ui.end_row();
                        // custom fields, two per row
                        if apps.custom_fields {
                            for (i, f) in fields.iter().enumerate() {
                                ui.label(RichText::new(format!("\u{25A4}  {}", f.name)).color(t.text2));
                                let v = task.custom.entry(f.id).or_default();
                                match f.kind.as_str() {
                                    "checkbox" => {
                                        let mut on = v == "true";
                                        if ui.checkbox(&mut on, "").changed() {
                                            *v = if on { "true".into() } else { String::new() };
                                        }
                                    }
                                    "dropdown" => {
                                        egui::ComboBox::from_id_salt(("cf", f.id)).selected_text(if v.is_empty() { "Empty".to_string() } else { v.clone() }).show_ui(ui, |ui| {
                                            ui.selectable_value(v, String::new(), "Empty");
                                            for o in &f.options {
                                                ui.selectable_value(v, o.clone(), o);
                                            }
                                        });
                                    }
                                    "date" => {
                                        let mut d = NaiveDate::parse_from_str(v, "%Y-%m-%d").ok();
                                        let r = ui.add(egui::Button::new(RichText::new(d.map(fmt_date).unwrap_or_else(|| "Empty".into())).color(if d.is_some() { t.text } else { t.text3 })).frame(false));
                                        if let Some(true) = popup(ui, &r, win_id.with(("cfd", f.id)), 380.0, |ui| date_menu(ui, &t, win_id.with(("cfdc", f.id)), &mut d)) {
                                            *v = d.map(|d| d.to_string()).unwrap_or_default();
                                        }
                                    }
                                    "number" => {
                                        let r = ui.add(egui::TextEdit::singleline(v).hint_text("Empty").desired_width(120.0));
                                        if r.changed() {
                                            v.retain(|c| c.is_ascii_digit() || c == '.' || c == '-');
                                        }
                                    }
                                    _ => {
                                        ui.add(egui::TextEdit::singleline(v).hint_text("Empty").desired_width(field_w - 120.0));
                                    }
                                }
                                if one_col || i % 2 == 1 {
                                    ui.end_row();
                                }
                            }
                            if !one_col && fields.len() % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
                    ui.add_space(14.0);
                    let r = ui.available_rect_before_wrap();
                    ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
                    ui.add_space(10.0);
                    ui.add(egui::TextEdit::multiline(&mut task.description).hint_text("Add description").frame(false).font(FontId::proportional(15.0)).desired_rows(4).desired_width(f32::INFINITY));
                    ui.add_space(16.0);
                    // subtasks
                    let subs: Vec<Task> = self.subtasks(tid).into_iter().cloned().collect();
                    let done = subs.iter().filter(|s| self.data.is_closed(s)).count();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Subtasks").strong());
                        if !subs.is_empty() {
                            ui.label(RichText::new(format!("{}/{}", done, subs.len())).color(t.text3));
                        }
                    });
                    for s in &subs {
                        let w = ui.available_width();
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 6.0, t.hover);
                        }
                        let st = self.data.status_of(s);
                        let cr = Rect::from_center_size(r.left_center() + vec2(14.0, 0.0), vec2(18.0, 18.0));
                        let c = ui.interact(cr, win_id.with(("subst", s.id)), Sense::click());
                        paint_status_circle(ui.painter(), cr.center(), 7.0, &st);
                        let mut se = s.clone();
                        let mut sub_changed = false;
                        if let Some(true) = popup(ui, &c, win_id.with(("substp", s.id)), 200.0, |ui| status_menu(ui, &t, &statuses, &mut se.status)) {
                            sub_changed = true;
                        }
                        let closed = matches!(st.kind, StatusKind::Closed | StatusKind::Done);
                        ui.painter().text(r.left_center() + vec2(32.0, 0.0), Align2::LEFT_CENTER, &s.name, FontId::proportional(13.0), if closed { t.text3 } else { t.text });
                        paint_avatars(ui.painter(), r.right_center() - vec2(110.0, 0.0), 10.0, &s.assignees, &self.data, t.card);
                        if let Some(d) = s.due {
                            ui.painter().text(r.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, fmt_date(d), FontId::proportional(12.0), date_color(d, closed, &t));
                        }
                        if sub_changed {
                            self.actions.push(Action::Replace(se));
                        } else if resp.clicked() {
                            self.actions.push(Action::Open(s.id));
                        }
                    }
                    // "Add subtask" row; the field appears only after a click
                    let sub_on_id = win_id.with(("adding_sub", tid));
                    let mut sub_on: bool = ui.data(|d| d.get_temp(sub_on_id)).unwrap_or(false);
                    if sub_on {
                        ui.horizontal(|ui| {
                            let r = ui.add(egui::TextEdit::singleline(&mut self.sub_text).hint_text("Task Name").desired_width(ui.available_width() - 70.0));
                            name_field(&r, "Subtask name");
                            focus_once(ui, &r);
                            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if (ui.button("Save").clicked() || enter) && !self.sub_text.trim().is_empty() {
                                self.actions.push(Action::Add { list: task.list_id, name: self.sub_text.clone(), status: None, priority: 0, assignees: vec![], due: None, parent: Some(tid), open: false });
                                self.sub_text.clear();
                                r.request_focus();
                            } else if r.lost_focus() && self.sub_text.trim().is_empty() {
                                sub_on = false;
                            }
                        });
                    } else if add_line(ui, &t, "+  Add subtask").clicked() {
                        sub_on = true;
                    }
                    ui.data_mut(|d| d.insert_temp(sub_on_id, sub_on));
                    ui.add_space(16.0);
                    // checklist
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Checklist").strong());
                        if !task.checklist.is_empty() {
                            let d = task.checklist.iter().filter(|c| c.done).count();
                            ui.label(RichText::new(format!("{}/{}", d, task.checklist.len())).color(t.text3));
                        }
                    });
                    if !task.checklist.is_empty() {
                        let d = task.checklist.iter().filter(|c| c.done).count() as f32 / task.checklist.len() as f32;
                        ui.add(egui::ProgressBar::new(d).desired_height(5.0).fill(rgb([0x00, 0x88, 0x44])));
                    }
                    let mut remove = None;
                    for (i, item) in task.checklist.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut item.done, "");
                            let mut txt = RichText::new(&item.text);
                            if item.done {
                                txt = txt.strikethrough().color(t.text3);
                            }
                            ui.label(txt);
                            if ui.add(egui::Button::new(RichText::new("\u{2716}").size(10.0).color(t.text3)).frame(false)).on_hover_text("Remove").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        task.checklist.remove(i);
                    }
                    let ck_on_id = win_id.with(("adding_check", tid));
                    let mut ck_on: bool = ui.data(|d| d.get_temp(ck_on_id)).unwrap_or(false);
                    if ck_on {
                        ui.horizontal(|ui| {
                            let r = ui.add(egui::TextEdit::singleline(&mut self.check_text).hint_text("Checklist item").desired_width(ui.available_width() - 70.0));
                            name_field(&r, "Checklist item");
                            focus_once(ui, &r);
                            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if (ui.button("Save").clicked() || enter) && !self.check_text.trim().is_empty() {
                                task.checklist.push(CheckItem { text: self.check_text.trim().to_string(), done: false });
                                self.check_text.clear();
                                r.request_focus();
                            } else if r.lost_focus() && self.check_text.trim().is_empty() {
                                ck_on = false;
                            }
                        });
                    } else if add_line(ui, &t, if task.checklist.is_empty() { "+  Create checklist" } else { "+  Add item" }).clicked() {
                        ck_on = true;
                    }
                    ui.data_mut(|d| d.insert_temp(ck_on_id, ck_on));
                    ui.add_space(30.0);
                });
                // ---- right: activity
                let mut right_ui = ui.new_child(egui::UiBuilder::new().max_rect(right.shrink2(vec2(16.0, 12.0))).layout(Layout::top_down(Align::Min)));
                right_ui.horizontal(|ui| {
                    ui.label(RichText::new("Comments").size(15.0).strong());
                    let created = chrono::DateTime::from_timestamp(task.created, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d").to_string()).unwrap_or_default();
                    ui.label(RichText::new(format!("Created {}", created)).color(t.text3));
                });
                right_ui.add_space(6.0);
                let comment_id = win_id.with("comment_box");
                // comment search (the magnifier next to the heading)
                let find_id = win_id.with(("comment_find", tid));
                let mut find: Option<String> = right_ui.data(|d| d.get_temp(find_id)).unwrap_or(None);
                if let Some(q) = find.as_mut() {
                    let mut drop = false;
                    right_ui.horizontal(|ui| {
                        let r = ui.add(egui::TextEdit::singleline(q).hint_text("Search comments...").desired_width(ui.available_width() - 30.0));
                        name_field(&r, "Search comments");
                        focus_once(ui, &r);
                        if ui.add(egui::Button::new("\u{2716}").frame(false)).on_hover_text("Close search").clicked() {
                            drop = true;
                        }
                    });
                    if drop {
                        find = None;
                    }
                }
                let has_comments = task.activity.iter().any(|a| a.comment);
                let mut assign_resp: Option<egui::Response> = None;
                if !has_comments {
                    let r = right_ui.available_rect_before_wrap();
                    let c = pos2(r.left() + 10.0, r.top() + r.height() * 0.32);
                    right_ui.painter().text(c, Align2::LEFT_BOTTOM, "Send the first comment", FontId::proportional(16.0), t.text);
                    right_ui.painter().text(c + vec2(0.0, 8.0), Align2::LEFT_TOP, "Ask a question, share an update, or loop someone in.", FontId::proportional(13.0), t.text2);
                    let row = Rect::from_min_size(c + vec2(0.0, 34.0), vec2(r.width() - 20.0, 30.0));
                    let mut bui = right_ui.new_child(egui::UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)));
                    if bui.button("@  Mention a teammate").clicked() {
                        if !self.comment_text.ends_with('@') {
                            if !self.comment_text.is_empty() && !self.comment_text.ends_with(' ') {
                                self.comment_text.push(' ');
                            }
                            self.comment_text.push('@');
                        }
                        bui.data_mut(|d| d.insert_temp(win_id.with("mention_open"), true));
                        bui.memory_mut(|m| m.request_focus(comment_id));
                    }
                    assign_resp = Some(bui.button("\u{1F464}  Assign this task"));
                }
                if let Some(r) = assign_resp {
                    let data = &self.data;
                    popup(&right_ui, &r, win_id.with("assign_popup"), 220.0, |ui| assignee_menu(ui, &t, data, &mut task.assignees));
                }
                let input_h = 150.0;
                let list_h = right_ui.available_height() - input_h;
                let q = find.as_ref().map(|q| q.trim().to_lowercase()).filter(|q| !q.is_empty());
                let only_id = win_id.with("comments_only");
                let mut only_comments: bool = right_ui.data(|d| d.get_temp(only_id)).unwrap_or(false);
                egui::ScrollArea::vertical().id_salt("activity").max_height(list_h).auto_shrink([false, false]).stick_to_bottom(true).show(&mut right_ui, |ui| {
                    if !has_comments {
                        return;
                    }
                    self.comment_feed(ui, &mut task, q.as_deref(), only_comments, win_id);
                });
                right_ui.add_space(6.0);
                // Comment / Post tabs, the box, then the tool row
                let tab_id = win_id.with("comment_tab");
                let mut post: bool = right_ui.data(|d| d.get_temp(tab_id)).unwrap_or(false);
                right_ui.horizontal(|ui| {
                    if ui.selectable_label(!post, "Comment").clicked() {
                        post = false;
                    }
                    if ui.selectable_label(post, "Post").clicked() {
                        post = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let r = ui.add(egui::Button::new("\u{1F50D}").frame(false));
                        name_it(&r, "Search comments button");
                        if r.on_hover_text("Search comments").clicked() {
                            find = if find.is_some() { None } else { Some(String::new()) };
                        }
                        // hide the "changed status..." lines
                        let only = ui.selectable_label(only_comments, RichText::new("Comments only").small());
                        name_it(&only, "Comments only");
                        if only.clicked() {
                            only_comments = !only_comments;
                        }
                    });
                });
                right_ui.data_mut(|d| {
                    d.insert_temp(only_id, only_comments);
                    d.insert_temp(tab_id, post);
                    d.insert_temp(find_id, find.clone());
                });
                // Cmd+Enter sends (taken before the box sees it as a new line)
                let send_key = right_ui.memory(|m| m.has_focus(comment_id)) && right_ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
                let cr = right_ui.add(egui::TextEdit::multiline(&mut self.comment_text).id(comment_id).hint_text(if post { "Write a post..." } else { "Write a comment, @mention someone" }).desired_rows(3).desired_width(f32::INFINITY));
                name_field(&cr, "Comment box");
                let mention_id = win_id.with("mention_open");
                // typing "@" (or a name after it) brings up the people list
                let typed_at = mention_word(&self.comment_text).map(str::to_string);
                if cr.changed() && typed_at.as_deref() == Some("") {
                    right_ui.data_mut(|d| d.insert_temp(mention_id, true));
                }
                let mut insert: Option<String> = None;
                right_ui.horizontal(|ui| {
                    let at = ui.add(egui::Button::new("@").frame(false).min_size(vec2(24.0, 24.0)));
                    name_it(&at, "Mention someone");
                    let force: bool = ui.data(|d| d.get_temp(mention_id)).unwrap_or(false);
                    // (opened a pass after the click, or that click would close it again)
                    if force && !ui.input(|i| i.pointer.any_click() || i.pointer.any_released()) {
                        ui.memory_mut(|m| m.open_popup(win_id.with("mention_popup")));
                        ui.data_mut(|d| d.remove::<bool>(mention_id));
                    }
                    let typed = typed_at.as_deref().unwrap_or("").to_lowercase();
                    let members: Vec<(u64, String)> = self.data.members.iter().filter(|m| !m.invited && m.name.to_lowercase().starts_with(&typed)).map(|m| (m.id, m.name.clone())).collect();
                    popup(ui, &at, win_id.with("mention_popup"), 200.0, |ui| {
                        if members.is_empty() {
                            ui.label(RichText::new("No one by that name").color(t.text3));
                        }
                        for (id, name) in &members {
                            let m = self.data.member(*id).cloned().unwrap_or_default();
                            let ring = t.card;
                            if menu_row(ui, &t, name, false, |p, c| paint_avatar(p, c, 8.0, &m, ring)).clicked() {
                                insert = Some(format!("@{} ", name));
                                close_popup(ui);
                            }
                        }
                    });
                    let em = ui.add(egui::Button::new("\u{263A}").frame(false).min_size(vec2(24.0, 24.0)));
                    name_it(&em, "Emoji");
                    popup(ui, &em, win_id.with("emoji_popup"), 180.0, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for e in ["\u{1F44D}", "\u{1F389}", "\u{2705}", "\u{1F440}", "\u{1F525}", "\u{1F64F}", "\u{1F604}", "\u{2764}", "\u{1F680}", "\u{1F914}", "\u{1F4AF}", "\u{1F44F}"] {
                                if ui.add(egui::Button::new(RichText::new(e).size(18.0)).frame(false)).clicked() {
                                    insert = Some(e.to_string());
                                    close_popup(ui);
                                }
                            }
                        });
                    });
                    let ck = ui.add(egui::Button::new("\u{2611}").frame(false).min_size(vec2(24.0, 24.0)));
                    name_it(&ck, "Checklist item in comment");
                    if ck.on_hover_text("Add a checklist item").clicked() {
                        let lead = if self.comment_text.is_empty() || self.comment_text.ends_with('\n') { "" } else { "\n" };
                        insert = Some(format!("{}\u{2610} ", lead));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let label = if post { "Post" } else { "Comment" };
                        let send = primary_button(ui, &t, label);
                        name_it(&send, &format!("Send {}", label));
                        if (send.clicked() || send_key) && !self.comment_text.trim().is_empty() {
                            let id = self.data.new_id();
                            task.activity.push(Activity { ts: now_ts(), author: self.data.me, text: self.comment_text.trim().to_string(), comment: true, post, id, ..Default::default() });
                            self.comment_text.clear();
                            close_popup(ui);
                        }
                    });
                });
                if let Some(text) = insert {
                    // a picked @name replaces the "@" (and part of a name) already typed
                    if text.starts_with('@') {
                        if let Some(w) = mention_word(&self.comment_text) {
                            let keep = self.comment_text.len() - w.len() - 1;
                            self.comment_text.truncate(keep);
                        }
                    }
                    self.comment_text.push_str(&text);
                    right_ui.memory_mut(|m| m.request_focus(comment_id));
                    let n = self.comment_text.chars().count();
                    let mut st = egui::TextEdit::load_state(right_ui.ctx(), comment_id).unwrap_or_default();
                    st.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(n))));
                    st.store(right_ui.ctx(), comment_id);
                }
            });
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, win_id));
        // Escape in a text field (subtask, checklist, comment) only leaves the field
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.menu_was_open && !self.typing_was_on {
            close = true;
        }
        // Cmd+1..4 set the priority, Cmd+0 clears it
        if self.data.extra.apps.priorities {
            for (k, p) in [(egui::Key::Num1, 1u8), (egui::Key::Num2, 2), (egui::Key::Num3, 3), (egui::Key::Num4, 4), (egui::Key::Num0, 0)] {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, k)) {
                    task.priority = p;
                }
            }
        }
        if task_changed(&orig, &task) {
            self.actions.push(Action::Replace(task));
        }
        if close {
            self.open_task = None;
        }
    }

    // ---------------------------------------------------------------- dialogs

    fn dialogs(&mut self, ctx: &egui::Context) {
        let t = self.t();
        // rename
        if let Some((tg, mut name)) = self.rename.take() {
            let mut open = true;
            let mut done = false;
            egui::Window::new("Rename").collapsible(false).resizable(false).anchor(Align2::CENTER_TOP, [0.0, 120.0]).show(ctx, |ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(320.0));
                r.request_focus();
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    done = true;
                }
                ui.horizontal(|ui| {
                    if primary_button(ui, &t, "Save").clicked() {
                        done = true;
                    }
                    if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        open = false;
                    }
                });
            });
            if done {
                self.finish_rename(tg, name);
            } else if open {
                self.rename = Some((tg, name));
            }
        }
        // delete confirmation
        if let Some(tg) = self.confirm_delete {
            let what = match tg {
                Target::Space(id) => format!("Space \"{}\"", self.data.location_name(Location::Space(id))),
                Target::Folder(id) => format!("Folder \"{}\"", self.data.location_name(Location::Folder(id))),
                Target::List(id) => format!("List \"{}\"", self.data.location_name(Location::List(id))),
                Target::Task(id) => format!("task \"{}\"", self.task(id).map(|x| x.name.clone()).unwrap_or_default()),
                Target::Workspace => "workspace".into(),
            };
            egui::Window::new("Delete").collapsible(false).resizable(false).anchor(Align2::CENTER_TOP, [0.0, 120.0]).show(ctx, |ui| {
                ui.label(format!("Delete {}? Everything inside it will be removed.", what));
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(RichText::new("Delete").color(Color32::WHITE)).fill(t.danger)).clicked() {
                        self.do_delete(tg);
                        self.confirm_delete = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_delete = None;
                    }
                });
            });
        }
        // statuses editor
        if let Some(sid) = self.edit_statuses {
            let mut open = true;
            if let Some(si) = self.data.spaces.iter().position(|s| s.id == sid) {
                let title = format!("Statuses - {}", self.data.spaces[si].name);
                let mut changed = false;
                egui::Window::new(title).open(&mut open).collapsible(false).resizable(false).anchor(Align2::CENTER_TOP, [0.0, 100.0]).show(ctx, |ui| {
                    let sts = &mut self.data.spaces[si].statuses;
                    let mut remove = None;
                    let n = sts.len();
                    for (i, st) in sts.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                            paint_status_circle(ui.painter(), r.center(), 7.0, st);
                            let pid = ui.id().with(("stc", i));
                            popup(ui, &resp, pid, 170.0, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    for c in STATUS_PALETTE {
                                        let (rr, cr) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                                        ui.painter().circle_filled(rr.center(), 8.0, rgb(c));
                                        if cr.clicked() {
                                            st.color = c;
                                            changed = true;
                                            close_popup(ui);
                                        }
                                    }
                                });
                            });
                            if ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(160.0)).changed() {
                                changed = true;
                            }
                            egui::ComboBox::from_id_salt(("kind", i)).selected_text(kind_label(st.kind)).width(90.0).show_ui(ui, |ui| {
                                for k in [StatusKind::Open, StatusKind::Custom, StatusKind::Done, StatusKind::Closed] {
                                    if ui.selectable_value(&mut st.kind, k, kind_label(k)).changed() {
                                        changed = true;
                                    }
                                }
                            });
                            if n > 1 && ui.add(egui::Button::new(RichText::new("\u{2716}").color(t.text3)).frame(false)).clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        sts.remove(i);
                        changed = true;
                    }
                    if ui.button("+ Add status").clicked() {
                        let c = STATUS_PALETTE[(sts.len() * 2 + 1) % STATUS_PALETTE.len()];
                        let pos = sts.iter().position(|s| matches!(s.kind, StatusKind::Done | StatusKind::Closed)).unwrap_or(sts.len());
                        sts.insert(pos, Status::new("new status", c, StatusKind::Custom));
                        changed = true;
                    }
                    ui.label(RichText::new("Tasks keep their status name; renamed statuses show as the first status.").small().color(t.text3));
                });
                if changed {
                    self.dirty = true;
                }
            }
            if !open {
                self.edit_statuses = None;
            }
        }
        // people
        if self.people_open {
            let mut open = true;
            egui::Window::new("People").open(&mut open).collapsible(false).resizable(false).anchor(Align2::CENTER_TOP, [0.0, 100.0]).show(ctx, |ui| {
                let mut remove = None;
                let me = self.data.me;
                for (i, m) in self.data.members.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                        paint_avatar(ui.painter(), r.center(), 11.0, m, t.card);
                        if ui.add(egui::TextEdit::singleline(&mut m.name).desired_width(180.0)).changed() {
                            self.dirty = true;
                        }
                        if m.id == me {
                            ui.label(RichText::new("you").small().color(t.text3));
                        } else if ui.add(egui::Button::new(RichText::new("\u{2716}").color(t.text3)).frame(false)).clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    let id = self.data.members[i].id;
                    self.data.members.remove(i);
                    for task in &mut self.data.tasks {
                        task.assignees.retain(|a| *a != id);
                    }
                    self.dirty = true;
                }
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.new_person).hint_text("Name").desired_width(180.0));
                    if ui.button("Add person").clicked() && !self.new_person.trim().is_empty() {
                        let id = self.data.new_id();
                        let color = STATUS_PALETTE[1 + (id as usize % 13)];
                        self.data.members.push(Member { id, name: self.new_person.trim().to_string(), color, role: "Member".into(), ..Default::default() });
                        self.new_person.clear();
                        self.dirty = true;
                    }
                });
            });
            if !open {
                self.people_open = false;
            }
        }
        // quick add (+ Create > Task)
        if let Some(mut qa) = self.quick_add.take() {
            let mut open = true;
            let mut create = false;
            let win_id = egui::Id::new("create_task_window");
            egui::Window::new("Create Task dialog")
                .id(win_id)
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_TOP, [0.0, 90.0])
                .min_width(600.0)
                .max_width(600.0)
                .frame(dialog_frame(&t))
                .show(ctx, |ui| {
                    if dialog_header(ui, &t, "Create Task") {
                        open = false;
                    }
                    ui.add_space(6.0);
                    // location
                    let mut all: Vec<(u64, String)> = vec![];
                    for s in self.data.spaces.iter().filter(|s| !s.archived) {
                        for l in s.lists.iter().filter(|l| !l.archived) {
                            all.push((l.id, format!("{} / {}", s.name, l.name)));
                        }
                        for f in &s.folders {
                            for l in f.lists.iter().filter(|l| !l.archived) {
                                all.push((l.id, format!("{} / {} / {}", s.name, f.name, l.name)));
                            }
                        }
                    }
                    let cur = all.iter().find(|x| x.0 == qa.list).map(|x| x.1.clone()).unwrap_or_else(|| "Choose list".into());
                    let before = qa.list;
                    egui::ComboBox::from_id_salt("qa_list").selected_text(truncate(&cur, 40)).show_ui(ui, |ui| {
                        for (id, name) in &all {
                            ui.selectable_value(&mut qa.list, *id, name);
                        }
                    });
                    if qa.list != before {
                        qa.status = None;
                    }
                    ui.add_space(6.0);
                    let r = ui.add(egui::TextEdit::singleline(&mut qa.name).hint_text("Task Name or type '/' for commands").font(FontId::proportional(18.0)).frame(false).desired_width(f32::INFINITY));
                    name_field(&r, "Task Name or type '/' for commands");
                    focus_once(ui, &r);
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        create = true;
                    }
                    ui.add_space(4.0);
                    let dr = ui.add(egui::TextEdit::multiline(&mut qa.description).hint_text("Add description").desired_rows(3).desired_width(f32::INFINITY));
                    name_field(&dr, "Add description");
                    ui.add_space(8.0);
                    // status, assignee, due date, priority, tags
                    let statuses = self.data.statuses_for_list(qa.list);
                    let mut st_name = qa.status.clone().unwrap_or_else(|| statuses.first().map(|s| s.name.clone()).unwrap_or_default());
                    ui.horizontal_wrapped(|ui| {
                        if let Some(st) = statuses.iter().find(|s| s.name == st_name).cloned() {
                            let r = status_pill(ui, &st, Sense::click());
                            name_it(&r, "Task status");
                            if popup(ui, &r, egui::Id::new("qa_st"), 200.0, |ui| status_menu(ui, &t, &statuses, &mut st_name)).unwrap_or(false) {
                                qa.status = Some(st_name.clone());
                                close_popup(ui);
                            }
                        }
                        let a_txt = if qa.assignees.is_empty() {
                            "\u{1F464} Assignee".to_string()
                        } else {
                            let names: Vec<String> = qa.assignees.iter().filter_map(|id| self.data.member(*id).map(|m| m.name.clone())).collect();
                            format!("\u{1F464} {}", truncate(&names.join(", "), 20))
                        };
                        let r = ui.button(a_txt);
                        let data = &self.data;
                        popup(ui, &r, egui::Id::new("qa_as"), 220.0, |ui| assignee_menu(ui, &t, data, &mut qa.assignees));
                        let d_txt = qa.due.map(|d| format!("\u{1F4C5} {}", fmt_date(d))).unwrap_or_else(|| "\u{1F4C5} Due date".into());
                        let r = ui.button(d_txt);
                        popup(ui, &r, egui::Id::new("qa_due"), 380.0, |ui| date_menu(ui, &t, egui::Id::new("qa_cal"), &mut qa.due));
                        let p_txt = if qa.priority > 0 { format!("\u{2691} {}", PRIORITIES[qa.priority as usize].label) } else { "\u{2691} Priority".into() };
                        let r = ui.button(p_txt);
                        popup(ui, &r, egui::Id::new("qa_pr"), 180.0, |ui| priority_menu(ui, &t, &mut qa.priority));
                        let r = ui.button("\u{1F3F7} Tags");
                        popup(ui, &r, egui::Id::new("qa_tags"), 220.0, |ui| {
                            let tr = ui.add(egui::TextEdit::singleline(&mut qa.tag_text).hint_text("Search or add tags...").desired_width(f32::INFINITY));
                            focus_once(ui, &tr);
                            if tr.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let tag = qa.tag_text.trim().to_lowercase();
                                if !tag.is_empty() && !qa.tags.contains(&tag) {
                                    qa.tags.push(tag);
                                }
                                qa.tag_text.clear();
                                tr.request_focus();
                            }
                            let mut known: Vec<String> = self.data.tasks.iter().flat_map(|t| t.tags.iter().cloned()).collect();
                            known.sort();
                            known.dedup();
                            let q = qa.tag_text.trim().to_lowercase();
                            for tag in known.iter().filter(|k| q.is_empty() || k.contains(&q)).take(8) {
                                let on = qa.tags.contains(tag);
                                if menu_row(ui, &t, tag, on, |p, c| {
                                    p.circle_filled(c, 4.0, tag_color(tag));
                                })
                                .clicked()
                                {
                                    if on {
                                        qa.tags.retain(|x| x != tag);
                                    } else {
                                        qa.tags.push(tag.clone());
                                    }
                                }
                            }
                        });
                        let mut drop: Option<String> = None;
                        for tag in &qa.tags {
                            if chip(ui, &format!("{} \u{00D7}", tag), tag_color(tag)).on_hover_text("Remove tag").clicked() {
                                drop = Some(tag.clone());
                            }
                        }
                        if let Some(d) = drop {
                            qa.tags.retain(|x| *x != d);
                        }
                    });
                    ui.add_space(10.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if primary_button(ui, &t, "Create Task").clicked() {
                                create = true;
                            }
                        });
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !self.menu_was_open {
                        open = false;
                    }
                });
            ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, win_id));
            if create && !qa.name.trim().is_empty() {
                let desc = qa.description.trim().to_string();
                if !desc.is_empty() {
                    self.pending_description = Some(desc);
                }
                self.pending_tags = std::mem::take(&mut qa.tags);
                self.actions.push(Action::Add { list: qa.list, name: qa.name.clone(), status: qa.status.clone(), priority: qa.priority, assignees: qa.assignees.clone(), due: qa.due, parent: None, open: false });
            } else if open {
                self.quick_add = Some(qa);
            }
        }
        // command search
        if let Some(mut q) = self.palette.take() {
            let mut open = true;
            egui::Window::new("search_palette").title_bar(false).collapsible(false).resizable(false).anchor(Align2::CENTER_TOP, [0.0, 60.0]).fixed_size([620.0, 420.0]).show(ctx, |ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Search, or jump to a task, list, doc or channel...").font(FontId::proportional(16.0)).desired_width(f32::INFINITY));
                r.request_focus();
                // kind chips, as in the original's search
                let kid = egui::Id::new("palette_kind");
                let mut kind: u8 = ui.data(|d| d.get_temp(kid)).unwrap_or(0);
                ui.horizontal(|ui| {
                    for (i, label) in ["All", "Tasks", "Lists", "Docs", "Channels", "Messages"].iter().enumerate() {
                        if ui.selectable_label(kind == i as u8, *label).clicked() {
                            kind = i as u8;
                        }
                    }
                });
                ui.data_mut(|d| d.insert_temp(kid, kind));
                ui.separator();
                let ql = q.to_lowercase();
                let mut first: Option<Action> = None;
                let mut open_doc: Option<u64> = None;
                let mut open_chat: Option<u64> = None;
                let show = |k: u8| kind == 0 || kind == k;
                egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                    if ql.is_empty() && kind == 0 {
                        let items = self.recent_items(None);
                        if !items.is_empty() {
                            ui.label(RichText::new("Recent").small().color(t.text3));
                            for (k, id, name, place) in items.into_iter().take(8) {
                                ui.horizontal(|ui| {
                                    let glyph = match k.as_str() {
                                        "task" => "\u{2611}",
                                        "doc" => "\u{1F4C4}",
                                        _ => "\u{2630}",
                                    };
                                    ui.label(RichText::new(glyph).color(t.text3));
                                    if ui.add(egui::Button::new(&name).frame(false)).clicked() {
                                        match k.as_str() {
                                            "task" => self.actions.push(Action::Open(id)),
                                            "doc" => open_doc = Some(id),
                                            _ => self.actions.push(Action::Go(Location::List(id))),
                                        }
                                        open = false;
                                    }
                                    ui.label(RichText::new(place).small().color(t.text3));
                                });
                            }
                            ui.add_space(4.0);
                        }
                    }
                    if show(2) {
                        let mut locs: Vec<(Location, String)> = vec![];
                        for s in &self.data.spaces {
                            locs.push((Location::Space(s.id), s.name.clone()));
                            for f in &s.folders {
                                locs.push((Location::Folder(f.id), format!("{} / {}", s.name, f.name)));
                                for l in &f.lists {
                                    locs.push((Location::List(l.id), format!("{} / {} / {}", s.name, f.name, l.name)));
                                }
                            }
                            for l in &s.lists {
                                locs.push((Location::List(l.id), format!("{} / {}", s.name, l.name)));
                            }
                        }
                        let locs: Vec<&(Location, String)> = locs.iter().filter(|(_, n)| ql.is_empty() || n.to_lowercase().contains(&ql)).take(if kind == 2 { 40 } else { 8 }).collect();
                        // like Docs and Channels: no heading over an empty group
                        if !locs.is_empty() {
                            ui.label(RichText::new("Locations").small().color(t.text3));
                        }
                        for (l, name) in locs {
                            if first.is_none() {
                                first = Some(Action::Go(*l));
                            }
                            if ui.add(egui::Button::new(name).frame(false)).clicked() {
                                self.actions.push(Action::Go(*l));
                                open = false;
                            }
                        }
                        ui.add_space(4.0);
                    }
                    if show(1) {
                        let tasks: Vec<&Task> = self.data.tasks.iter().filter(|x| !x.archived && (ql.is_empty() || x.name.to_lowercase().contains(&ql) || x.id.to_string() == ql)).take(if kind == 1 { 60 } else { 20 }).collect();
                        if !tasks.is_empty() {
                            ui.label(RichText::new("Tasks").small().color(t.text3));
                        }
                        for task in tasks {
                            if first.is_none() {
                                first = Some(Action::Open(task.id));
                            }
                            ui.horizontal(|ui| {
                                let st = self.data.status_of(task);
                                let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                                paint_status_circle(ui.painter(), r.center(), 6.0, &st);
                                if ui.add(egui::Button::new(&task.name).frame(false)).clicked() {
                                    self.actions.push(Action::Open(task.id));
                                    open = false;
                                }
                                ui.label(RichText::new(self.data.list_name(task.list_id)).small().color(t.text3));
                            });
                        }
                        ui.add_space(4.0);
                    }
                    if show(3) {
                        let docs: Vec<(u64, String)> = self.data.docs.iter().filter(|d| !d.archived && (ql.is_empty() || d.title.to_lowercase().contains(&ql) || (ql.len() > 2 && d.body.to_lowercase().contains(&ql)))).map(|d| (d.id, d.title.clone())).take(20).collect();
                        if !docs.is_empty() {
                            ui.label(RichText::new("Docs").small().color(t.text3));
                            for (id, title) in docs {
                                if ui.add(egui::Button::new(format!("\u{1F4C4}  {}", title)).frame(false)).clicked() {
                                    open_doc = Some(id);
                                    open = false;
                                }
                            }
                            ui.add_space(4.0);
                        }
                    }
                    if show(4) {
                        let chans: Vec<(u64, String)> = self.data.channels.iter().filter(|c| c.kind == ChannelKind::Channel && (ql.is_empty() || c.name.to_lowercase().contains(&ql))).map(|c| (c.id, c.name.clone())).take(20).collect();
                        if !chans.is_empty() {
                            ui.label(RichText::new("Channels").small().color(t.text3));
                            for (id, name) in chans {
                                if ui.add(egui::Button::new(format!("#  {}", name)).frame(false)).clicked() {
                                    open_chat = Some(id);
                                    open = false;
                                }
                            }
                            ui.add_space(4.0);
                        }
                    }
                    if show(5) && ql.len() > 1 {
                        let mut hits: Vec<(u64, String, String)> = vec![];
                        for c in &self.data.channels {
                            for m in &c.messages {
                                if m.text.to_lowercase().contains(&ql) {
                                    hits.push((c.id, if c.name.is_empty() { "Direct message".into() } else { format!("#{}", c.name) }, m.text.clone()));
                                }
                            }
                        }
                        if !hits.is_empty() {
                            ui.label(RichText::new("Messages").small().color(t.text3));
                            for (cid, where_, text) in hits.into_iter().take(20) {
                                ui.horizontal(|ui| {
                                    if ui.add(egui::Button::new(truncate(&text, 60)).frame(false)).clicked() {
                                        open_chat = Some(cid);
                                        open = false;
                                    }
                                    ui.label(RichText::new(where_).small().color(t.text3));
                                });
                            }
                        }
                    }
                });
                if let Some(id) = open_doc {
                    self.doc_open = Some(id);
                    self.section = Section::Docs;
                }
                if let Some(id) = open_chat {
                    self.section = Section::Chat;
                    self.chat_open = Some(id);
                    self.thread_open = None;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Some(a) = first {
                        self.actions.push(a);
                    }
                    open = false;
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

    /// One frame of the whole interface (everything except the native window buttons).
    pub(crate) fn ui_frame(&mut self, ctx: &egui::Context) {
        self.menu_was_open = any_menu_was_open(ctx);
        self.drafts.chat.swap(self.chat_open, &mut self.chat_text);
        self.drafts.thread.swap(self.thread_open, &mut self.thread_text);
        self.drafts.chat_files.swap(self.chat_open, &mut self.chat_files);
        self.drafts.thread_files.swap(self.thread_open, &mut self.thread_files);
        self.files_tick(Some(ctx));
        self.personal_list_defaults();
        self.sync_view_prefs();
        if let Ok(p) = std::env::var("WORKBOARD_EVENT_LOG") {
            let evs: Vec<String> = ctx.input(|i| i.raw.events.iter().filter(|e| !matches!(e, egui::Event::PointerMoved(_) | egui::Event::MouseMoved(_))).map(|e| format!("{:?}", e)).collect());
            if !evs.is_empty() {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
                    for e in evs {
                        let _ = writeln!(f, "{}", e);
                    }
                }
            }
        }
        if self.data.extra.auto_theme {
            if let Some(sys) = ctx.input(|i| i.raw.system_theme) {
                self.data.dark = sys == egui::Theme::Dark;
            }
        }
        let t = self.t();
        theme::apply(ctx, &t);
        // shortcuts
        if ctx.input(|i| i.modifiers.command && (i.key_pressed(egui::Key::K) || i.key_pressed(egui::Key::J))) {
            self.palette = if self.palette.is_some() { None } else { Some(String::new()) };
        }
        if ctx.input(|i| (i.modifiers.command && i.key_pressed(egui::Key::N)) || (i.modifiers.alt && i.key_pressed(egui::Key::T))) {
            self.open_quick_add(None);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::G)) {
            self.new_message_open = true;
        }
        self.single_key_shortcuts(ctx);
        self.top_bar(ctx);
        self.rail(ctx);
        self.content(ctx);
        self.task_window(ctx);
        self.dialogs(ctx);
        self.modals(ctx);
        self.media_viewer(ctx);
        self.notepad(ctx);
        self.new_message_window(ctx);
        self.bulk_bar(ctx);
        self.toast_ui(ctx);
        self.apply_actions();
        self.track_history();
        self.note_recent();
        if self.dirty {
            // lines logged with `..Default::default()` get their ids before saving
            self.data.fix_activity_ids();
            self.sync_note_change();
        }
        if self.dirty && self.persist {
            if self.ws_file == "data.json" {
                self.data.save();
            } else {
                self.data.save_file(&self.ws_file);
            }
        }
        self.dirty = false;
        if self.persist {
            self.sync_tick(ctx);
        }
        self.huddle_tick(Some(ctx));
        self.huddle_bar(ctx);
        // a joined workspace needs to know who you are
        // (on a server the invite code says who you are)
        if self.data.extra.shared.as_ref().is_some_and(|c| c.server.is_none()) && self.data.member(self.data.me).is_none() && self.modal.is_none() {
            self.modal = Some(Modal::WhoAmI { name: String::new(), email: String::new() });
        }
        self.handle_screenshot(ctx);
        // read at the end of the pass: egui drops the focus on Escape before
        // the next pass reaches our code
        self.typing_was_on = ctx.memory(|m| m.focused()).is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
    }

    fn handle_snapshot(&mut self, ctx: &egui::Context) {
        // also on request: a `snap-request` file next to the data file (used when
        // trying the app with a separate WORKBOARD_HOME folder)
        let mut pressed = ctx.input(|i| i.modifiers.ctrl && i.modifiers.alt && i.modifiers.mac_cmd && i.key_pressed(egui::Key::S));
        if std::env::var("WORKBOARD_HOME").map_or(false, |v| !v.is_empty()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
            let req = Data::dir().join("snap-request");
            if req.exists() {
                let _ = std::fs::remove_file(&req);
                pressed = true;
            }
        }
        if pressed && !self.snap_pending {
            self.snap_pending = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            return;
        }
        if !self.snap_pending {
            return;
        }
        ctx.request_repaint();
        let img = ctx.input(|i| i.raw.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }));
        if let Some(img) = img {
            self.snap_pending = false;
            let [w, h] = img.size;
            let mut buf = Vec::with_capacity(w * h * 4);
            for p in &img.pixels {
                buf.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
            }
            let path = Data::dir().join("snapshot.png");
            if let Some(im) = image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                let _ = std::fs::create_dir_all(Data::dir());
                let _ = im.save(&path);
            }
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        if self.screenshot.is_none() {
            self.handle_snapshot(ctx);
        }
        let Some(path) = self.screenshot.clone() else { return };
        self.frames += 1;
        ctx.request_repaint();
        if self.frames >= 120 && !self.shot_sent {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.shot_sent = true;
        }
        let images: Vec<std::sync::Arc<egui::ColorImage>> = ctx.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
                .collect()
        });
        if let Some(img) = images.into_iter().next() {
            let [w, h] = img.size;
            let mut buf = Vec::with_capacity(w * h * 4);
            for p in &img.pixels {
                buf.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
            }
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Some(im) = image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                let _ = im.save(&path);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.frames > 600 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        place_traffic_lights(frame);
        #[cfg(not(target_os = "macos"))]
        let _ = frame;
        self.ui_frame(ctx);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.catch_paste(ctx, raw_input);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.persist {
            // stop a running timer cleanly so tracked time is kept
            let now = now_ts();
            for t in &mut self.data.tasks {
                if let Some(st) = t.timer_started.take() {
                    t.time_entries.push(TimeEntry { start: st, end: now });
                }
            }
            self.data.save();
        }
    }
}

/// Day offsets beyond this are grouped as "Future" (original: B$4 = 7).
const DUE_GROUP_CAP: i64 = 7;

/// Due-date group key, same rule as the original's grouping function.
fn due_group_key(due: Option<NaiveDate>, closed: bool) -> String {
    if closed {
        return "done".into();
    }
    let Some(d) = due else { return "null".into() };
    let i = (d - today()).num_days();
    if i == 0 {
        "0".into()
    } else if i > 0 {
        if i >= DUE_GROUP_CAP { format!("+{}", DUE_GROUP_CAP) } else { format!("{}", i) }
    } else {
        "-1".into()
    }
}

/// Move the macOS window buttons into the 40 px header (inset 14 px from the
/// left, centred vertically), the way the original's desktop window does.
#[cfg(target_os = "macos")]
fn place_traffic_lights(frame: &eframe::Frame) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSPoint {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSSize {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSRect {
        origin: NSPoint,
        size: NSSize,
    }
    let Ok(handle) = frame.window_handle() else { return };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return };
    unsafe {
        let view = h.ns_view.as_ptr() as *mut Object;
        let window: *mut Object = msg_send![view, window];
        if window.is_null() {
            return;
        }
        let close: *mut Object = msg_send![window, standardWindowButton: 0u64];
        let mini: *mut Object = msg_send![window, standardWindowButton: 1u64];
        let zoom: *mut Object = msg_send![window, standardWindowButton: 2u64];
        if close.is_null() || mini.is_null() || zoom.is_null() {
            return;
        }
        let titlebar: *mut Object = msg_send![close, superview];
        if titlebar.is_null() {
            return;
        }
        let container: *mut Object = msg_send![titlebar, superview];
        if container.is_null() {
            return;
        }
        let close_rect: NSRect = msg_send![close, frame];
        // the buttons sit at the bottom of this strip: centre them 20 px from the top
        let height = 20.0 + close_rect.size.height / 2.0;
        let win_rect: NSRect = msg_send![window, frame];
        let mut c_rect: NSRect = msg_send![container, frame];
        if (c_rect.size.height - height).abs() > 0.5 || (c_rect.origin.y - (win_rect.size.height - height)).abs() > 0.5 {
            c_rect.size.height = height;
            c_rect.origin.y = win_rect.size.height - height;
            let _: () = msg_send![container, setFrame: c_rect];
        }
        // close at 13 px from the left edge, buttons 20 px apart
        let space = 20.0;
        for (i, b) in [close, mini, zoom].iter().enumerate() {
            let r: NSRect = msg_send![*b, frame];
            let origin = NSPoint { x: 13.0 + space * i as f64, y: r.origin.y };
            if (r.origin.x - origin.x).abs() > 0.5 || (r.origin.y - origin.y).abs() > 0.5 {
                let _: () = msg_send![*b, setFrameOrigin: origin];
            }
        }
    }
}

/// The name being typed after an "@" at the end of a comment ("" right
/// after the "@"), if the text ends in one.
fn mention_word(text: &str) -> Option<&str> {
    let at = text.rfind('@')?;
    let w = &text[at + 1..];
    let starts_word = text[..at].chars().last().map_or(true, char::is_whitespace);
    (starts_word && !w.contains(char::is_whitespace)).then_some(w)
}

fn task_changed(a: &Task, b: &Task) -> bool {
    a.name != b.name
        || a.status != b.status
        || a.priority != b.priority
        || a.assignees != b.assignees
        || a.due != b.due
        || a.start != b.start
        || a.description != b.description
        || a.tags != b.tags
        || a.estimate_min != b.estimate_min
        || a.recur != b.recur
        || a.activity.len() != b.activity.len()
        || a.activity.iter().zip(b.activity.iter()).any(|(x, y)| x.id != y.id || x.text != y.text || x.edited != y.edited || x.reactions != y.reactions || x.assignee != y.assignee || x.resolved != y.resolved)
        || a.checklist.len() != b.checklist.len()
        || a.custom.iter().filter(|(_, v)| !v.is_empty()).ne(b.custom.iter().filter(|(_, v)| !v.is_empty()))
        || a.checklist.iter().zip(b.checklist.iter()).any(|(x, y)| x.done != y.done || x.text != y.text)
}

fn kind_label(k: StatusKind) -> &'static str {
    match k {
        StatusKind::Open => "Not started",
        StatusKind::Custom => "Active",
        StatusKind::Done => "Done",
        StatusKind::Closed => "Closed",
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut o: String = s.chars().take(n.saturating_sub(1)).collect();
        o.push('\u{2026}');
        o
    }
}

fn td_default() -> NaiveDate {
    today()
}

pub(crate) fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // The original uses the system UI font (-apple-system / Segoe UI); use it when present.
    for p in ["/System/Library/Fonts/SFNS.ttf", "C:\\Windows\\Fonts\\segoeui.ttf"] {
        if let Ok(bytes) = std::fs::read(p) {
            fonts.font_data.insert("system-ui".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "system-ui".into());
            // a fallback for monospace text: the shortcut keys use symbols such
            // as the Option key that the built-in monospace font does not have
            fonts.families.entry(egui::FontFamily::Monospace).or_default().push("system-ui".into());
            break;
        }
    }
    let candidates = [
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        "/System/Library/Fonts/Supplemental/AppleGothic.ttf",
        "C:\\Windows\\Fonts\\malgun.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    ];
    for p in candidates {
        if let Ok(bytes) = std::fs::read(p) {
            fonts.font_data.insert("cjk".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(fam).or_default().push("cjk".into());
            }
            break;
        }
    }
    ctx.set_fonts(fonts);
}

#[allow(dead_code)]
fn _unused(_: HashMap<u8, u8>) {}
