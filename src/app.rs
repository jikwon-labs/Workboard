// The application window: global top bar, sidebar hierarchy, view tabs
// (List, Board, Calendar, Gantt, Timeline, Table), Home and Inbox pages, the task
// detail window, quick add and the command search.

use crate::i18n::{tr, DateText};
use crate::model::*;
use crate::theme::{self, rgb, Theme};
use crate::timefmt::{local, local_date, local_ts, nine_am, reminder_ts};
use crate::trf;
use crate::widgets::*;
use chrono::{Datelike, Duration, NaiveDate};
use egui::{pos2, vec2, Align, Align2, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, Ui};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[cfg(test)]
mod account_tests;
mod chat;
mod comments;
mod delete_ws;
mod docs;
#[cfg(test)]
mod due_time_tests;
mod files;
mod hubs;
mod huddle;
mod icons;
mod listview;
mod login;
mod menus;
mod more;
mod nav;
mod notify;
#[cfg(test)]
mod notify_tests;
mod pages;
mod parity;
mod settings;
mod sharing;
mod shell;
mod slack;
mod task_window;
mod taskfields;
mod templates;
#[cfg(test)]
mod tests;
mod timebar;
mod timeline;
mod together;
#[cfg(test)]
mod ui_tests;
use menus::Modal;
pub use pages::Section;
use parity::TaskFilter;
pub use taskfields::TaskPick;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    Overview,
    List,
    Board,
    Calendar,
    Gantt,
    Timeline,
    Table,
}

impl ViewKind {
    const ALL: [ViewKind; 7] = [
        ViewKind::Overview,
        ViewKind::List,
        ViewKind::Board,
        ViewKind::Calendar,
        ViewKind::Gantt,
        ViewKind::Timeline,
        ViewKind::Table,
    ];
    fn label(self) -> &'static str {
        match self {
            ViewKind::Overview => "Overview",
            ViewKind::List => "List",
            ViewKind::Board => "Board",
            ViewKind::Calendar => "Calendar",
            ViewKind::Gantt => "Gantt",
            ViewKind::Timeline => "Timeline",
            ViewKind::Table => "Table",
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
    const ALL: [GroupBy; 7] = [
        GroupBy::Status,
        GroupBy::Priority,
        GroupBy::Assignee,
        GroupBy::DueDate,
        GroupBy::Tags,
        GroupBy::List,
        GroupBy::None,
    ];
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
    const ALL: [SortBy; 10] = [
        SortBy::Manual,
        SortBy::Status,
        SortBy::Name,
        SortBy::Assignee,
        SortBy::Priority,
        SortBy::DueDate,
        SortBy::StartDate,
        SortBy::Created,
        SortBy::Updated,
        SortBy::Estimate,
    ];
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
}

pub enum Action {
    Open(u64),
    Replace(Task),
    Add {
        list: u64,
        name: String,
        status: Option<String>,
        priority: u8,
        assignees: Vec<u64>,
        due: Option<NaiveDate>,
        parent: Option<u64>,
        open: bool,
    },
    DeleteTask(u64),
    Go(Location),
    ToggleSpace(u64),
    ToggleFolder(u64),
    #[cfg_attr(not(test), allow(dead_code))] // only tests send it
    NewSpace,
    NewFolder(u64),
    #[cfg_attr(not(test), allow(dead_code))] // only tests send it
    NewList {
        space: u64,
        folder: Option<u64>,
    },
    StartRename(Target),
    AskDelete(Target),
    ToggleTimer(u64),
    /// Stops a timer an older app started without recording who.
    StopOldTimer(u64),
    ToggleFavorite(Location),
    EditStatuses(u64),
    SetSpaceColor(u64, [u8; 3]),
}

/// A column of the Table view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TableCol {
    Num,
    Name,
    Status,
    Assignee,
    Due,
    Priority,
    Tags,
    Tracked,
    Created,
    Field(u64),
}

impl TableCol {
    /// The columns that can be hidden, in their order (custom fields follow).
    const FIXED: [TableCol; 7] = [
        TableCol::Status,
        TableCol::Assignee,
        TableCol::Due,
        TableCol::Priority,
        TableCol::Tags,
        TableCol::Tracked,
        TableCol::Created,
    ];
    fn label(self) -> &'static str {
        match self {
            TableCol::Num => "#",
            TableCol::Name => "Name",
            TableCol::Status => "Status",
            TableCol::Assignee => "Assignee",
            TableCol::Due => "Due date",
            TableCol::Priority => "Priority",
            TableCol::Tags => "Tags",
            TableCol::Tracked => "Time tracked",
            TableCol::Created => "Date created",
            TableCol::Field(_) => "",
        }
    }
    fn width(self) -> f32 {
        match self {
            TableCol::Num => 44.0,
            TableCol::Name => 300.0,
            TableCol::Status => 130.0,
            TableCol::Tags => 150.0,
            TableCol::Tracked => 100.0,
            TableCol::Field(_) => 140.0,
            _ => 110.0,
        }
    }
    /// Its entry in `hidden_cols` (kept apart from the List's columns).
    fn key(self) -> String {
        match self {
            TableCol::Field(id) => format!("table:field:{}", id),
            c => format!("table:{}", c.label()),
        }
    }
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
        DraftBox {
            on: None,
            saved: HashMap::new(),
        }
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
        *text = now
            .and_then(|id| self.saved.remove(&id))
            .unwrap_or_default();
        self.on = now;
    }
}

#[derive(Default)]
struct Drafts {
    chat: DraftBox,
    thread: DraftBox,
    chat_files: DraftBox<Vec<files::Pending>>,
    thread_files: DraftBox<Vec<files::Pending>>,
    chat_picked: DraftBox<Vec<u64>>,
    thread_picked: DraftBox<Vec<u64>>,
}

/// What a task's list says about it: whose private space it's in, whether
/// it or its folder or space is archived, and its space's statuses.
struct ListInfo<'a> {
    private_owner: Option<u64>,
    archived: bool,
    statuses: &'a [Status],
}

/// Every list's `ListInfo`, for going over many tasks at once (looking each
/// task's list up on its own walks all the spaces every time).
fn list_infos(d: &Data) -> HashMap<u64, ListInfo<'_>> {
    let mut out = HashMap::new();
    for s in &d.spaces {
        let info = |l: &ListItem, folder_archived: bool| ListInfo {
            private_owner: s.private_owner,
            archived: l.archived || folder_archived || s.archived,
            statuses: &s.statuses,
        };
        for l in &s.lists {
            out.entry(l.id).or_insert_with(|| info(l, false));
        }
        for f in &s.folders {
            for l in &f.lists {
                out.entry(l.id).or_insert_with(|| info(l, f.archived));
            }
        }
    }
    out
}

/// `Data::is_closed` with the lists looked up already.
fn closed_in(info: &HashMap<u64, ListInfo>, t: &Task) -> bool {
    let sts = info
        .get(&t.list_id)
        .map_or(default_statuses_ref(), |l| l.statuses);
    let st = sts
        .iter()
        .find(|s| s.name == t.status)
        .or(sts.first())
        .unwrap_or(&default_statuses_ref()[0]);
    matches!(st.kind, StatusKind::Closed | StatusKind::Done)
}

/// See `App::task_index`.
#[derive(Default)]
pub(super) struct TaskIndex {
    /// The workspace's change count and number of tasks it was made for.
    key: Option<(u64, usize)>,
    at: HashMap<u64, usize>,
    children: HashMap<u64, Vec<usize>>,
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
    /// The open status editor's bookkeeping: (space, the names tasks carry
    /// for each status (None: added in the editor), the names automations
    /// were last pointed at). See `commit_status_renames`.
    status_edit: Option<(u64, Vec<Option<String>>, Vec<Option<String>>)>,
    palette: Option<String>,
    quick_add: Option<QuickAdd>,
    comment_text: String,
    /// People picked from the @mention list while writing a comment, so a
    /// name two people share means the one picked.
    comment_picked: Vec<u64>,
    /// People picked from the @mention list in the channel's message box
    /// (kept with its draft), so a name two people share means the one
    /// picked.
    chat_picked: Vec<u64>,
    /// The same for the open thread's reply box (kept with its draft).
    thread_picked: Vec<u64>,
    /// How many messages the open conversation draws after "Show earlier
    /// messages" (by conversation; `CHAT_PAGE` for any other).
    chat_window: Option<(u64, usize)>,
    sub_text: String,
    check_text: String,
    tag_text: String,
    dirty: bool,
    persist: bool,
    /// Changes not saved yet: when the first and the latest of them were
    /// made (see `save_tick`).
    save_due: std::cell::Cell<Option<(std::time::Instant, std::time::Instant)>>,
    /// Goes up when the workspace changes (`data_changed`).
    data_gen: std::cell::Cell<u64>,
    /// The Inbox rows and counts asked for on every frame (nav.rs).
    inbox_cache: std::cell::RefCell<nav::InboxCache>,
    /// Tasks by id and by parent (`task_index`).
    task_index: std::cell::RefCell<TaskIndex>,
    screenshot: Option<PathBuf>,
    frames: u32,
    shot_sent: bool,
    /// Ctrl+Option+Cmd+S: save a picture of the window next to the data file.
    snap_pending: bool,
    actions: Vec<Action>,
    home_collapsed: HashSet<&'static str>,
    /// Home "My Work" sections showing every task instead of the first few.
    home_all: HashSet<&'static str>,
    section: Section,
    recent: Vec<u64>,
    doc_open: Option<u64>,
    planner_week: NaiveDate,
    sheet_week: NaiveDate,
    chat_open: Option<u64>,
    nav_last: Option<nav::NavSpot>,
    nav_back: Vec<nav::NavSpot>,
    nav_fwd: Vec<nav::NavSpot>,
    nav_skip: bool,
    assignee_filter: Option<u64>,
    show_search: bool,
    // focus the toolbar search box once, on the frame it opens
    search_focus: bool,
    side_search_focus: bool,
    subtasks_open: bool,
    modal: Option<Modal>,
    /// The sign-in screen, filling the window in place of the app (see login.rs).
    login: Option<login::LoginScreen>,
    notepad_open: bool,
    toast: Option<(String, i64)>,
    sync_rt: together::SyncRt,
    /// A workspace being made on the team server (see together.rs).
    making_ws: Option<together::MakingWs>,
    /// a delete asked of the team server, waiting for its answer
    deleting_ws: Option<delete_ws::DeletingWs>,
    huddle: huddle::HuddleRt,
    notify_rt: notify::NotifyRt,
    /// Where each workspace file lives (team server, shared folder, this computer), read once.
    ws_places: HashMap<String, &'static str>,
    create_text: String,
    new_message_open: bool,
    pending_template: Option<TaskTemplate>,
    pending_description: Option<String>,
    /// A history line for the next task added (a form answer says which form).
    pending_log: Option<String>,
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
    /// The doc whose open was last counted (Popular Wikis views).
    doc_viewed: Option<u64>,
    temp_app: Option<Section>,
    team_open: Option<u64>,
    sheet_entries: bool,
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
    timeline: timeline::TimelineState,
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
    /// Pictures and videos on their way onto a task: (task, file). Kept in
    /// the cache folder too (`taskfields::TASK_FILES`), so ones still going
    /// up when the app quits join their task after the next start.
    task_files: Vec<(u64, files::Pending)>,
    /// What of `task_files` is written down (task, file), once read back
    /// for this workspace; None until then.
    task_files_saved: Option<Vec<(u64, String)>>,
    /// Page state of the apps (reminders, search, whiteboards, forms, ...).
    sweep: more::SweepUi,
    /// Interface language picked in Settings > Language (kept on this computer).
    lang_choice: crate::i18n::Choice,
    /// The task or message (channel, message) the Reminder window is for.
    reminder_about: (Option<u64>, Option<(u64, u64)>),
    /// A workboard:// link clicked in a message, opened after the pass.
    link_pending: Option<String>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        setup_fonts(&cc.egui_ctx);
        crate::links::wake(&cc.egui_ctx);
        Self::build(screenshot, None)
    }

    /// Builds the app state; `data` is used as-is (and never saved) when given.
    pub(crate) fn build(screenshot: Option<PathBuf>, data: Option<Data>) -> Self {
        let persist = screenshot.is_none() && data.is_none();
        // tests and screenshots are in English; the app follows Settings > Language
        let mut lang_choice = if persist {
            Data::language()
        } else {
            crate::i18n::Choice::En
        };
        // --lang ko / --lang en: for screenshots, without changing the saved choice
        let args: Vec<String> = std::env::args().collect();
        if let Some(l) = args
            .iter()
            .position(|a| a == "--lang")
            .and_then(|i| args.get(i + 1))
        {
            lang_choice = crate::i18n::choice_from_key(l);
        }
        crate::i18n::set_lang(crate::i18n::resolve(lang_choice));
        // reopen the workspace that was open last (a joined team workspace, say)
        let last = if persist {
            Data::last_workspace().filter(|f| f != "data.json")
        } else {
            None
        };
        let mut ws_file = "data.json".to_string();
        let data = match data {
            Some(d) => d,
            None if persist => match last
                .as_deref()
                .and_then(|f| Data::load_file(f).map(|d| (f, d)))
            {
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
        data.fold_legacy_slack();
        together::fill_sync_baseline(&mut data);
        // updates come from the team's server even while a workspace kept
        // only on this computer is open
        if persist {
            if let Some(addr) = together::update_server(&data) {
                crate::updater::set_server(&addr);
            }
        }
        let loc = data
            .first_list()
            .map(Location::List)
            .unwrap_or(Location::Home);
        let t = today();
        let args: Vec<String> = std::env::args().collect();
        let arg = |k: &str| {
            args.iter()
                .position(|a| a == k)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        let view = match arg("--view").as_deref() {
            Some("board") => ViewKind::Board,
            Some("calendar") => ViewKind::Calendar,
            Some("gantt") => ViewKind::Gantt,
            Some("timeline") => ViewKind::Timeline,
            Some("table") => ViewKind::Table,
            _ => ViewKind::List,
        };
        let loc = match arg("--page").as_deref() {
            Some("home") => Location::Home,
            Some("inbox") => Location::Inbox,
            Some("everything") => Location::Everything,
            _ => loc,
        };
        let open_task = if args.iter().any(|a| a == "--open-task") {
            data.tasks.first().map(|t| t.id)
        } else {
            None
        };
        // the sign-in screen: a first start, or logged out last time
        // (--modal welcome / signin / join show its steps for screenshots)
        let start_login = match arg("--modal").as_deref() {
            Some("welcome") => Some(login::LoginScreen::start("")),
            Some("signin") => Some(login::LoginScreen::start("macmini")),
            Some("join") => Some(login::LoginScreen::new(login::LoginPage::Invite, "macmini")),
            _ if persist && data.extra.welcome => {
                Some(login::LoginScreen::start(&login::last_server()))
            }
            _ => None,
        };
        let start_modal = match arg("--modal").as_deref() {
            Some("invite") => Some(Modal::Invite {
                emails: String::new(),
                role: "Member".into(),
            }),
            Some("automations") => Some(Modal::Automations {
                status: "complete".into(),
                action: "comment".into(),
                value: String::new(),
                scope: None,
            }),
            Some("fields") => Some(Modal::CustomFields {
                name: String::new(),
                kind: "text".into(),
                opts: String::new(),
            }),
            Some("themes") => Some(Modal::Themes),
            Some("together") => Some(Modal::Together {
                path: String::new(),
                addr: String::new(),
                code: String::new(),
                result: String::new(),
            }),
            Some("call") => Some(Modal::Call {
                ch: data.channels.first().map(|c| c.id).unwrap_or(0),
                link: String::new(),
            }),
            Some("reminder") => Some(Modal::Reminder {
                text: String::new(),
                date: Some(today()),
                time: "09:00".into(),
            }),
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
            status_edit: None,
            palette: None,
            quick_add: None,
            comment_text: String::new(),
            comment_picked: vec![],
            chat_picked: vec![],
            thread_picked: vec![],
            chat_window: None,
            sub_text: String::new(),
            check_text: String::new(),
            tag_text: String::new(),
            dirty: false,
            persist,
            save_due: std::cell::Cell::new(None),
            data_gen: std::cell::Cell::new(0),
            inbox_cache: Default::default(),
            task_index: Default::default(),
            screenshot,
            frames: 0,
            shot_sent: false,
            snap_pending: false,
            actions: vec![],
            home_collapsed: ["Done"].into_iter().collect(),
            home_all: HashSet::new(),
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
            login: start_login,
            notepad_open: false,
            // shown a little longer than usual, as the window may still be opening
            toast: if persist {
                Data::take_unreadable().map(|p| (unreadable_note(&p), now_ts() + 10))
            } else {
                None
            },
            sync_rt: Default::default(),
            making_ws: None,
            deleting_ws: None,
            huddle: Default::default(),
            notify_rt: Default::default(),
            ws_places: HashMap::new(),
            create_text: String::new(),
            new_message_open: false,
            pending_template: None,
            pending_description: None,
            pending_log: None,
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
            doc_viewed: None,
            temp_app: None,
            team_open: None,
            sheet_entries: false,
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
            files: files::Files::new(if persist {
                Data::dir().join("files")
            } else {
                std::env::temp_dir().join(format!("wb-files-{:x}", wb_sync::random_u64()))
            }),
            viewer: None,
            pasted: None,
            paste_text_seen: false,
            new_channel: None,
            selected: HashSet::new(),
            menu_was_open: false,
            typing_was_on: false,
            filter: TaskFilter::default(),
            hidden_cols: std::iter::once("Date created")
                .chain(parity::PROPERTIES)
                .map(String::from)
                .collect(),
            fields_open: false,
            list_width: 0.0,
            filter_open: false,
            personal_saved: None,
            sort_desc: false,
            cal_week: false,
            gantt_zoom: 0,
            gantt_jump: true,
            timeline: Default::default(),
            doc_preview: false,
            settings_tab: 0,
            settings_state: None,
            comment_drafts: HashMap::new(),
            comment_for: None,
            docs_search: String::new(),
            docs_tag: None,
            view_prefs: if persist {
                listview::load_prefs()
            } else {
                HashMap::new()
            },
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
            task_files: vec![],
            task_files_saved: None,
            sweep: Default::default(),
            lang_choice,
            reminder_about: (None, None),
            link_pending: None,
        }
    }

    fn t(&self) -> Theme {
        theme::theme_accent(self.data.dark, &self.data.extra.accent)
    }

    /// Show `msg` in the toast at the bottom of the window for a few seconds.
    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), now_ts()));
    }

    // ---------------------------------------------------------------- data helpers

    /// Where each task is (by id) and which tasks are under each parent, so
    /// views can look tasks up for every row on every frame. Made again
    /// once the workspace changed (`data_changed`) or the number of tasks
    /// did; until then a stale entry can only miss, never give the wrong
    /// task, since every lookup checks what it found.
    fn task_index(&self) -> std::cell::Ref<'_, TaskIndex> {
        let key = (self.data_gen.get(), self.data.tasks.len());
        if self.task_index.borrow().key != Some(key) {
            let mut ix = self.task_index.borrow_mut();
            ix.key = Some(key);
            ix.at.clear();
            ix.children.clear();
            for (i, t) in self.data.tasks.iter().enumerate() {
                ix.at.insert(t.id, i);
                if let Some(p) = t.parent {
                    ix.children.entry(p).or_default().push(i);
                }
            }
        }
        self.task_index.borrow()
    }

    fn task(&self, id: u64) -> Option<&Task> {
        let found = {
            let ix = self.task_index();
            match ix.at.get(&id) {
                Some(&i) => Some(i),
                // the index is up to date and has no such task
                None if !self.dirty => return None,
                None => None,
            }
        };
        if let Some(t) = found.and_then(|i| self.data.tasks.get(i)) {
            if t.id == id {
                return Some(t);
            }
        }
        // a change not taken in yet moved things: look the slow way
        self.data.tasks.iter().find(|t| t.id == id)
    }

    fn subtasks(&self, id: u64) -> Vec<&Task> {
        // archived subtasks only show with the Archived filter on
        let shows = |t: &Task| t.parent == Some(id) && (self.filter.archived || !t.archived);
        let mut v: Vec<&Task> = if self.dirty {
            // a change not taken in yet may have moved one under it
            self.data.tasks.iter().filter(|t| shows(t)).collect()
        } else {
            let ix = self.task_index();
            ix.children
                .get(&id)
                .into_iter()
                .flatten()
                .filter_map(|&i| self.data.tasks.get(i))
                .filter(|t| shows(t))
                .collect()
        };
        v.sort_by_key(|t| t.order);
        v
    }

    /// Tasks matching the current location and filters (top level only,
    /// plus the subtasks a search finds).
    fn filtered(&self) -> Vec<&Task> {
        let lists = self.data.lists_in(self.loc);
        let q = self.search.to_lowercase();
        // each list's space and statuses, found once instead of per task
        let info = list_infos(&self.data);
        let mut v: Vec<&Task> = self
            .data
            .tasks
            .iter()
            // subtasks show nested under their parent, except: my assigned
            // subtasks on the personal views, and archived subtasks of a
            // task that isn't archived when the Archived filter is on
            .filter(|t| {
                t.parent.is_none()
                    || (matches!(self.loc, Location::MyTasks | Location::TodayOverdue) && t.assignees.contains(&self.data.me))
                    || (self.filter.archived && t.archived && !t.parent.and_then(|p| self.task(p)).is_some_and(|p| p.archived))
                    // a search finds subtasks too
                    || !q.is_empty()
            })
            .filter(|t| match self.loc {
                Location::MyTasks => t.assignees.contains(&self.data.me),
                Location::TodayOverdue => t.assignees.contains(&self.data.me) && t.due.map(|d| d <= today()).unwrap_or(false) && !self.data.is_closed(t),
                _ => lists.as_ref().map(|l| l.contains(&t.list_id)).unwrap_or(true),
            })
            .filter(|t| q.is_empty() || t.name.to_lowercase().contains(&q) || t.tags.iter().any(|g| g.to_lowercase().contains(&q)))
            .filter(|t| !self.me_mode || t.assignees.contains(&self.data.me))
            .filter(|t| info.get(&t.list_id).and_then(|l| l.private_owner).map(|o| o == self.data.me).unwrap_or(true))
            .filter(|t| matches!(self.loc, Location::List(_)) || !info.get(&t.list_id).is_some_and(|l| l.archived))
            .filter(|t| self.assignee_filter.map(|a| t.assignees.contains(&a)).unwrap_or(true))
            .filter(|t| self.show_closed || self.filter.archived || !closed_in(&info, t))
            .filter(|t| self.filter.matches(t, &self.data))
            .collect();
        self.sort(&mut v);
        v
    }

    /// Not archived, not in an archived list or space and not in someone
    /// else's private space: the tasks personal counts and Home show.
    pub(crate) fn task_visible(&self, t: &Task) -> bool {
        !t.archived
            && !self.data.list_archived(t.list_id)
            && !self
                .data
                .space_of_list(t.list_id)
                .and_then(|s| s.private_owner)
                .is_some_and(|o| o != self.data.me)
    }

    fn sort(&self, v: &mut [&Task]) {
        match self.sort_by {
            SortBy::Manual => v.sort_by_key(|t| t.order),
            // by day, then by time within it (a day without one last)
            SortBy::DueDate => v.sort_by_key(|t| (t.due.is_none(), t.due_moment(), t.order)),
            SortBy::Priority => {
                v.sort_by_key(|t| (if t.priority == 0 { 9 } else { t.priority }, t.order))
            }
            // each name lowered once, not on every comparison
            SortBy::Name => v.sort_by_cached_key(|t| t.name.to_lowercase()),
            SortBy::Created => v.sort_by_key(|t| std::cmp::Reverse(t.created)),
            SortBy::Updated => v.sort_by_key(|t| std::cmp::Reverse(t.updated)),
            // a day without a time first (it starts with the day), then by time
            SortBy::StartDate => {
                v.sort_by_key(|t| (t.start.is_none(), t.start, t.start_at(), t.order))
            }
            SortBy::Estimate => {
                v.sort_by_key(|t| (t.estimate_min.is_none(), t.estimate_min, t.order))
            }
            SortBy::Status => {
                // in the order the statuses are set up (open first, closed last)
                let order: Vec<String> = self
                    .location_statuses()
                    .into_iter()
                    .map(|s| s.name)
                    .collect();
                v.sort_by_cached_key(|t| {
                    (
                        order
                            .iter()
                            .position(|n| *n == self.data.status_ref(t).name)
                            .unwrap_or(usize::MAX),
                        t.order,
                    )
                });
            }
            SortBy::Assignee => {
                // by the first assignee's name; unassigned last
                let key = |t: &Task| {
                    t.assignees
                        .iter()
                        .filter_map(|a| self.data.member(*a))
                        .map(|m| m.name.to_lowercase())
                        .min()
                };
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
            Location::Folder(fid) => self
                .data
                .spaces
                .iter()
                .filter(|s| s.folders.iter().any(|f| f.id == fid))
                .collect(),
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
        self.groups_by(self.group_by, tasks, include_empty)
    }

    /// The groups `tasks` fall into under `by` (the Timeline's lanes use
    /// their own choice, apart from the List's Group setting).
    fn groups_by(&self, by: GroupBy, tasks: &[&Task], include_empty: bool) -> Vec<Group> {
        let t = self.t();
        let mut out: Vec<Group> = vec![];
        match by {
            GroupBy::Status => {
                for st in self.location_statuses() {
                    if !self.show_closed
                        && matches!(st.kind, StatusKind::Closed | StatusKind::Done)
                        && !include_empty
                    {
                        continue;
                    }
                    let ids: Vec<u64> = tasks
                        .iter()
                        .filter(|x| self.data.status_ref(x).name == st.name)
                        .map(|x| x.id)
                        .collect();
                    if ids.is_empty() && !include_empty {
                        continue;
                    }
                    out.push(Group {
                        key: format!("s:{}", st.name),
                        label: crate::i18n::status_text(&st.name).to_uppercase(),
                        color: rgb(st.color),
                        status: Some(st),
                        priority: None,
                        assignee: None,
                        due: None,
                        tasks: ids,
                    });
                }
            }
            GroupBy::Priority => {
                for pr in [1u8, 2, 3, 4, 0] {
                    let ids: Vec<u64> = tasks
                        .iter()
                        .filter(|x| x.priority == pr)
                        .map(|x| x.id)
                        .collect();
                    if ids.is_empty() && !include_empty {
                        continue;
                    }
                    out.push(Group {
                        key: format!("p:{}", pr),
                        label: tr(PRIORITIES[pr as usize].label).to_uppercase(),
                        color: priority_color(pr, &t),
                        status: None,
                        priority: Some(pr),
                        assignee: None,
                        due: None,
                        tasks: ids,
                    });
                }
            }
            GroupBy::Assignee => {
                for m in &self.data.members {
                    let ids: Vec<u64> = tasks
                        .iter()
                        .filter(|x| x.assignees.contains(&m.id))
                        .map(|x| x.id)
                        .collect();
                    if ids.is_empty() && !include_empty {
                        continue;
                    }
                    out.push(Group {
                        key: format!("a:{}", m.id),
                        label: m.name.to_uppercase(),
                        color: rgb(m.color),
                        status: None,
                        priority: None,
                        assignee: Some(m.id),
                        due: None,
                        tasks: ids,
                    });
                }
                let ids: Vec<u64> = tasks
                    .iter()
                    .filter(|x| x.assignees.is_empty())
                    .map(|x| x.id)
                    .collect();
                if !ids.is_empty() || include_empty {
                    out.push(Group {
                        key: "a:none".into(),
                        label: tr("UNASSIGNED").into(),
                        color: t.text3,
                        status: None,
                        priority: None,
                        assignee: None,
                        due: None,
                        tasks: ids,
                    });
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
                // each task's bucket worked out once, not once per bucket
                let task_keys: Vec<String> = tasks
                    .iter()
                    .map(|x| due_group_key(x.due, self.data.is_closed(x)))
                    .collect();
                for k in keys {
                    if k == "done" && !self.show_closed && !include_empty {
                        continue;
                    }
                    let ids: Vec<u64> = tasks
                        .iter()
                        .zip(&task_keys)
                        .filter(|(_, tk)| **tk == k)
                        .map(|(x, _)| x.id)
                        .collect();
                    if ids.is_empty() && !include_empty {
                        continue;
                    }
                    let (label, due, color) = match k.as_str() {
                        "-1" => (
                            tr("OVERDUE").to_string(),
                            Some(td - Duration::days(1)),
                            t.danger,
                        ),
                        "0" => (
                            tr("TODAY").to_string(),
                            Some(td),
                            Color32::from_rgb(0xe1, 0x6b, 0x16),
                        ),
                        "1" => (
                            tr("TOMORROW").to_string(),
                            Some(td + Duration::days(1)),
                            rgb([0x10, 0x90, 0xe0]),
                        ),
                        "null" => (tr("NO DUE DATE").to_string(), None, t.text3),
                        "done" => (tr("DONE").to_string(), None, rgb([0x00, 0x88, 0x44])),
                        _ if k.starts_with('+') => (
                            tr("FUTURE").to_string(),
                            Some(td + Duration::days(DUE_GROUP_CAP)),
                            rgb([0x5f, 0x55, 0xee]),
                        ),
                        _ => {
                            let n: i64 = k.parse().unwrap_or(2);
                            let d = td + Duration::days(n);
                            (
                                d.text("%A").to_uppercase(),
                                Some(d),
                                rgb([0x44, 0x66, 0xff]),
                            )
                        }
                    };
                    out.push(Group {
                        key: format!("d:{}", k),
                        label,
                        color,
                        status: None,
                        priority: None,
                        assignee: None,
                        due,
                        tasks: ids,
                    });
                }
            }
            GroupBy::Tags => {
                // a task with several tags shows under each of them
                let mut tags: Vec<String> =
                    tasks.iter().flat_map(|x| x.tags.iter().cloned()).collect();
                tags.sort_by_key(|g| g.to_lowercase());
                tags.dedup();
                for tag in tags {
                    let ids: Vec<u64> = tasks
                        .iter()
                        .filter(|x| x.tags.contains(&tag))
                        .map(|x| x.id)
                        .collect();
                    out.push(Group {
                        key: format!("t:{}", tag),
                        label: tag.to_uppercase(),
                        color: tag_color(&tag),
                        status: None,
                        priority: None,
                        assignee: None,
                        due: None,
                        tasks: ids,
                    });
                }
                let ids: Vec<u64> = tasks
                    .iter()
                    .filter(|x| x.tags.is_empty())
                    .map(|x| x.id)
                    .collect();
                if !ids.is_empty() || include_empty {
                    out.push(Group {
                        key: "t:".into(),
                        label: tr("NO TAGS").into(),
                        color: t.text3,
                        status: None,
                        priority: None,
                        assignee: None,
                        due: None,
                        tasks: ids,
                    });
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
                    let ids: Vec<u64> = tasks
                        .iter()
                        .filter(|x| x.list_id == l)
                        .map(|x| x.id)
                        .collect();
                    if ids.is_empty() && !include_empty {
                        continue;
                    }
                    let color = self
                        .data
                        .list_item(l)
                        .and_then(|li| li.color)
                        .map(rgb)
                        .unwrap_or(t.text2);
                    out.push(Group {
                        key: format!("l:{}", l),
                        label: self.data.list_name(l).to_uppercase(),
                        color,
                        status: None,
                        priority: None,
                        assignee: None,
                        due: None,
                        tasks: ids,
                    });
                }
            }
            GroupBy::None => {
                out.push(Group {
                    key: "all".into(),
                    label: tr("ALL TASKS").into(),
                    color: t.solid(),
                    status: None,
                    priority: None,
                    assignee: None,
                    due: None,
                    tasks: tasks.iter().map(|x| x.id).collect(),
                });
            }
        }
        out
    }

    /// The list new tasks go to from the current location.
    fn default_list(&self) -> Option<u64> {
        match self.loc {
            // (a List that is gone: somewhere else)
            Location::List(id) => (self.data.space_of_list(id).is_some())
                .then_some(id)
                .or_else(|| self.data.first_list()),
            _ => self
                .data
                .lists_in(self.loc)
                .and_then(|v| v.into_iter().find(|l| !self.data.list_archived(*l)))
                .or_else(|| self.data.first_list()),
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
                    if self
                        .data
                        .statuses_for_list(task.list_id)
                        .iter()
                        .any(|s| s.name == st.name)
                    {
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

    /// Adds a task in the group; false (and the next step offered) when
    /// there is no List for it.
    fn add_from_group(&mut self, g: &Group, name: String, open: bool) -> bool {
        let Some(list) = Self::group_list(g).or_else(|| self.default_list()) else {
            self.need_a_list();
            return false;
        };
        if let Some(tag) = Self::group_tag(g).filter(|x| !x.is_empty()) {
            self.pending_tags.push(tag.to_string());
        }
        let mut assignees = vec![];
        if let Some(a) = g.assignee {
            assignees.push(a);
        }
        // on the personal views a new task is mine (and due today on Today &
        // Overdue), or it would leave the page as soon as it's made
        let personal = matches!(self.loc, Location::MyTasks | Location::TodayOverdue);
        if personal && !assignees.contains(&self.data.me) {
            // with Multiple Assignees off a task has one: me
            if !self.data.extra.apps.multiple_assignees {
                assignees.clear();
            }
            assignees.push(self.data.me);
        }
        let due = if self.loc == Location::TodayOverdue {
            g.due.filter(|d| *d <= today()).or(Some(today()))
        } else {
            g.due
        };
        self.actions.push(Action::Add {
            list,
            name,
            status: g.status.as_ref().map(|s| s.name.clone()),
            priority: g.priority.unwrap_or(0),
            assignees,
            due,
            parent: None,
            open,
        });
        true
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
        task.activity.push(Activity {
            ts: now_ts(),
            author: me,
            text,
            comment: false,
            ..Default::default()
        });
    }

    /// A history line about one person ("assigned to <name>", "removed
    /// assignee <name>") that keeps their id, so their Inbox finds it by id.
    pub(super) fn about_line(&self, me: u64, what: &str, who: u64) -> Activity {
        let name = self
            .data
            .member(who)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        Activity {
            ts: now_ts(),
            author: me,
            text: format!("{} {}", what, name),
            comment: false,
            target: Some(who),
            ..Default::default()
        }
    }

    /// Logs a change, or updates my own line about the same change if I made
    /// it in the last few minutes (`like` is part of that line).
    fn log_edit(task: &mut Task, me: u64, like: &str, text: String) {
        let now = now_ts();
        if let Some(last) = task.activity.last_mut() {
            if !last.comment && last.author == me && now - last.ts < 300 && last.text.contains(like)
            {
                last.text = text;
                last.ts = now;
                return;
            }
        }
        Self::log(task, me, text);
    }

    fn apply(&mut self, a: Action) {
        let me = self.data.me;
        // a space shared with me to view or comment only (the server would put it back)
        if let Some(why) = self.refused_here(&a) {
            self.toast(tr(why));
            return;
        }
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
                    new.tidy_times(&old);
                    if old.status != new.status {
                        let m = format!(
                            "changed status from {} to {}",
                            old.status.to_uppercase(),
                            new.status.to_uppercase()
                        );
                        Self::log(&mut new, me, m);
                        // Date done / Date closed: when it entered such a status
                        let kind = |t: &Task| self.data.status_of(t).kind;
                        let (was, now_k) = (kind(&old), kind(&new));
                        let done =
                            |k: StatusKind| matches!(k, StatusKind::Done | StatusKind::Closed);
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
                        let m = format!(
                            "set priority to {}",
                            PRIORITIES[new.priority as usize].label
                        );
                        Self::log(&mut new, me, m);
                    }
                    // the history is shared with teammates: keep it in English
                    // with a time: "Nov 16 at 2:30 PM"
                    let fmt_date = |d: NaiveDate, time: Option<chrono::NaiveTime>| {
                        crate::i18n::in_english(|| match time {
                            Some(time) => format!("{} at {}", fmt_date(d), fmt_time(time)),
                            None => fmt_date(d),
                        })
                    };
                    if (old.due, old.due_at()) != (new.due, new.due_at()) {
                        let s = new
                            .due
                            .map(|d| fmt_date(d, new.due_at()))
                            .unwrap_or_else(|| "none".into());
                        Self::log(&mut new, me, format!("changed due date to {}", s));
                    }
                    if (old.start, old.start_at()) != (new.start, new.start_at()) {
                        let s = new
                            .start
                            .map(|d| fmt_date(d, new.start_at()))
                            .unwrap_or_else(|| "none".into());
                        Self::log(&mut new, me, format!("changed start date to {}", s));
                    }
                    for aid in new.assignees.clone() {
                        if !old.assignees.contains(&aid) {
                            new.activity.push(self.about_line(me, "assigned to", aid));
                        }
                    }
                    for &aid in &old.assignees {
                        if !new.assignees.contains(&aid) {
                            new.activity
                                .push(self.about_line(me, "removed assignee", aid));
                        }
                    }
                    // typed fields change on every key press: while I keep typing,
                    // the line I logged a moment ago is updated instead of adding more
                    if old.name != new.name && !new.name.trim().is_empty() {
                        let m = format!("renamed this task to \"{}\"", new.name.trim());
                        Self::log_edit(&mut new, me, "renamed this task to ", m);
                    }
                    if old.description != new.description {
                        Self::log_edit(
                            &mut new,
                            me,
                            "updated the description",
                            "updated the description".into(),
                        );
                    }
                    if old.estimate_min != new.estimate_min {
                        let m = match new.estimate_min {
                            Some(min) => format!(
                                "set the time estimate to {}",
                                crate::i18n::in_english(|| fmt_duration(min as i64 * 60))
                            ),
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
                    if old.status != new.status
                        && self.data.extra.labs.iter().any(|l| l == "confetti")
                    {
                        let closes = self.data.statuses_for_list(new.list_id).iter().any(|s| {
                            s.name == new.status
                                && matches!(s.kind, StatusKind::Closed | StatusKind::Done)
                        });
                        if closes {
                            self.toast(trf!("\u{1F389} Nice! \"{}\" is complete", new.name));
                        }
                    }
                    let mut move_to = None;
                    if old.status != new.status {
                        let space = self.data.space_of_list(new.list_id).map(|s| s.id);
                        let rules: Vec<Automation> = self
                            .data
                            .extra
                            .automations
                            .iter()
                            .filter(|a| {
                                a.enabled
                                    && a.trigger_status == new.status
                                    && a.list.is_none_or(|l| l == new.list_id || Some(l) == space)
                            })
                            .cloned()
                            .collect();
                        for a in rules {
                            match a.action.as_str() {
                                "assign" => {
                                    if let Ok(uid) = a.value.parse::<u64>() {
                                        if !self.data.extra.apps.multiple_assignees {
                                            new.assignees = vec![uid];
                                        } else if !new.assignees.contains(&uid) {
                                            new.assignees.push(uid);
                                            new.activity.push(self.about_line(
                                                me,
                                                "assigned to",
                                                uid,
                                            ));
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
                                            // moved below, with subtasks and statuses, as Move does
                                            move_to = Some(l);
                                        }
                                    }
                                }
                                _ => new.activity.push(Activity {
                                    ts: now_ts(),
                                    author: me,
                                    text: a.value.clone(),
                                    comment: true,
                                    mentions: self.data.mentions_in(&a.value),
                                    ..Default::default()
                                }),
                            }
                            Self::log(
                                &mut new,
                                me,
                                format!("Automation ran: {}", self.describe_action(&a)),
                            );
                        }
                    }
                    self.roll_recurring(&old, &mut new);
                    new.updated = now_ts();
                    let nid = new.id;
                    self.data.tasks[idx] = new;
                    self.dirty = true;
                    if let Some(l) = move_to {
                        self.move_tasks(&[nid], l);
                    }
                }
            }
            Action::Add {
                list,
                name,
                status,
                priority,
                assignees,
                due,
                parent,
                open,
            } => {
                let id = self.data.new_id();
                let statuses = self.data.statuses_for_list(list);
                let status = status.unwrap_or_else(|| statuses[0].name.clone());
                let order = self.data.tasks.iter().map(|t| t.order).max().unwrap_or(0) + 1;
                let now = now_ts();
                // people put on the task as it is made hear about it like a later assignment
                let mut activity = vec![Activity {
                    ts: now,
                    author: me,
                    text: "created this task".into(),
                    comment: false,
                    ..Default::default()
                }];
                activity.extend(
                    assignees
                        .iter()
                        .filter(|a| **a != me)
                        .map(|a| self.about_line(me, "assigned to", *a)),
                );
                self.data.tasks.push(Task {
                    id,
                    list_id: list,
                    parent,
                    name: if name.trim().is_empty() {
                        tr("Untitled task").into()
                    } else {
                        name.trim().to_string()
                    },
                    description: String::new(),
                    status,
                    priority,
                    assignees,
                    start: None,
                    due,
                    start_time: None,
                    due_time: None,
                    tags: vec![],
                    checklist: vec![],
                    activity,
                    time_entries: vec![],
                    timer_started: None,
                    timer_by: None,
                    timers: vec![],
                    estimate_min: None,
                    created: now,
                    updated: now,
                    order,
                    custom: Default::default(),
                    recur: None,
                    archived: false,
                    done_at: None,
                    closed_at: None,
                    milestone: false,
                    watchers: vec![],
                    waiting_on: vec![],
                    linked: vec![],
                    attachments: vec![],
                });
                if let Some(tp) = self.pending_template.take() {
                    if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == id) {
                        t.description = tp.description.clone();
                        t.tags = tp.tags.clone();
                        t.checklist = tp
                            .checklist
                            .iter()
                            .map(|c| CheckItem {
                                text: c.clone(),
                                done: false,
                                assignee: None,
                            })
                            .collect();
                    }
                }
                if let Some(desc) = self.pending_description.take() {
                    if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == id) {
                        t.description = desc;
                    }
                }
                if let Some(line) = self.pending_log.take() {
                    if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == id) {
                        Self::log(t, me, line);
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
                    kill.extend(
                        self.data
                            .tasks
                            .iter()
                            .filter(|t| t.parent == Some(k))
                            .map(|t| t.id),
                    );
                    i += 1;
                }
                let removed: Vec<Task> = self
                    .data
                    .tasks
                    .iter()
                    .filter(|t| kill.contains(&t.id))
                    .cloned()
                    .collect();
                if let Some(first) = removed.iter().find(|t| t.id == id) {
                    let tid = self.data.new_id();
                    let title = first.name.clone();
                    self.data.extra.trash.push(TrashItem {
                        id: tid,
                        kind: "Task".into(),
                        title,
                        deleted: now_ts(),
                        payload: serde_json::to_string(&removed).unwrap_or_default(),
                    });
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
                if !matches!(
                    l,
                    Location::Home
                        | Location::Inbox
                        | Location::Replies
                        | Location::Assigned
                        | Location::MyTasks
                        | Location::TodayOverdue
                ) && !self.data.location_exists(l)
                {
                    return;
                }
                self.section = Section::Home;
                if matches!(l, Location::Space(_))
                    && self.loc != l
                    && !self.data.extra.hidden_views.iter().any(|h| h == "Overview")
                {
                    // spaces open on their Overview, as in the original
                    self.view = ViewKind::Overview;
                } else if self.view == ViewKind::Overview
                    && !matches!(l, Location::Space(_) | Location::Folder(_))
                {
                    // Overview only covers a space or a folder
                    self.view = ViewKind::List;
                }
                if self.loc != l {
                    self.selected.clear();
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
                self.data.spaces.push(Space {
                    id,
                    name: tr("New Space").into(),
                    color,
                    statuses: default_statuses(),
                    folders: vec![],
                    lists: vec![ListItem {
                        id: lid,
                        name: tr("List").into(),
                        ..Default::default()
                    }],
                    expanded: true,
                    private_owner: None,
                    ..Default::default()
                });
                self.rename = Some((Target::Space(id), tr("New Space").into()));
                self.dirty = true;
            }
            Action::NewFolder(sid) => {
                let id = self.data.new_id();
                let lid = self.data.new_id();
                if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == sid) {
                    s.folders.push(Folder {
                        id,
                        name: tr("New Folder").into(),
                        lists: vec![ListItem {
                            id: lid,
                            name: tr("List").into(),
                            ..Default::default()
                        }],
                        expanded: true,
                        archived: false,
                    });
                    s.expanded = true;
                }
                self.rename = Some((Target::Folder(id), tr("New Folder").into()));
                self.dirty = true;
            }
            Action::NewList { space, folder } => {
                let id = self.data.new_id();
                if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == space) {
                    s.expanded = true;
                    let item = ListItem {
                        id,
                        name: tr("New List").into(),
                        ..Default::default()
                    };
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
                self.rename = Some((Target::List(id), tr("New List").into()));
                self.dirty = true;
            }
            Action::StartRename(tg) => {
                let name = match tg {
                    Target::Space(id) => self.data.location_name(Location::Space(id)),
                    Target::Folder(id) => self.data.location_name(Location::Folder(id)),
                    Target::List(id) => self.data.location_name(Location::List(id)),
                    Target::Task(id) => self.task(id).map(|t| t.name.clone()).unwrap_or_default(),
                };
                self.rename = Some((tg, name));
            }
            Action::AskDelete(tg) => self.confirm_delete = Some(tg),
            Action::ToggleTimer(id) => {
                let now = now_ts();
                let Some(task) = self.task(id) else { return };
                // one timer per person per task: teammates' timers keep running
                if self.data.is_my_timer(task) {
                    self.data.stop_my_timer(id, now);
                } else {
                    // I track one task at a time
                    self.data.stop_my_timers(now);
                    self.data.start_my_timer(id, now);
                }
                self.dirty = true;
            }
            Action::StopOldTimer(id) => {
                self.data.stop_ownerless_timer(id, now_ts());
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
            Action::EditStatuses(sid) => {
                // another space's editor: the open one's edits are done first
                if self.edit_statuses != Some(sid) {
                    self.finish_status_edit();
                }
                self.edit_statuses = Some(sid);
            }
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
                    for l in s
                        .lists
                        .iter_mut()
                        .chain(s.folders.iter_mut().flat_map(|f| f.lists.iter_mut()))
                    {
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
                // the lists inside go to Trash one by one, like deleting a
                // list, and the space itself after them, so it can come back
                let mut entries = vec![];
                for &l in &lists {
                    self.trash_list(l);
                    entries.extend(self.last_trash_id());
                }
                if let Some(mut space) = self.data.spaces.iter().find(|s| s.id == id).cloned() {
                    space.lists.clear();
                    for f in &mut space.folders {
                        f.lists.clear();
                    }
                    let title = space.name.clone();
                    let payload = serde_json::to_string(&SpaceTrash {
                        space,
                        lists: entries,
                    })
                    .unwrap_or_default();
                    let tid = self.data.new_id();
                    self.data.extra.trash.push(TrashItem {
                        id: tid,
                        kind: "Space".into(),
                        title,
                        deleted: now_ts(),
                        payload,
                    });
                }
                self.data.spaces.retain(|s| s.id != id);
            }
            Target::Folder(id) => {
                if let Some(l) = self.data.lists_in(Location::Folder(id)) {
                    lists = l;
                }
                let mut entries = vec![];
                for &l in &lists {
                    self.trash_list(l);
                    entries.extend(self.last_trash_id());
                }
                let found = self.data.spaces.iter().find_map(|s| {
                    s.folders
                        .iter()
                        .find(|f| f.id == id)
                        .map(|f| (s.id, f.clone()))
                });
                if let Some((space, mut folder)) = found {
                    folder.lists.clear();
                    let title = folder.name.clone();
                    let payload = serde_json::to_string(&FolderTrash {
                        space,
                        folder,
                        lists: entries,
                    })
                    .unwrap_or_default();
                    let tid = self.data.new_id();
                    self.data.extra.trash.push(TrashItem {
                        id: tid,
                        kind: "Folder".into(),
                        title,
                        deleted: now_ts(),
                        payload,
                    });
                }
                for s in &mut self.data.spaces {
                    s.folders.retain(|f| f.id != id);
                }
            }
            Target::List(id) => {
                lists = vec![id];
                self.trash_list(id);
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
        }
        self.data.tasks.retain(|t| !lists.contains(&t.list_id));
        let data = &self.data;
        self.data.favorites = data
            .favorites
            .iter()
            .copied()
            .filter(|f| data.location_exists(*f))
            .collect();
        if !self.data.location_exists(self.loc) {
            self.loc = Location::Home;
        }
        self.dirty = true;
    }

    /// Passes status renames typed in the status editor on to the space's
    /// tasks as they are typed. `orig` holds the name tasks carry for each
    /// status (None: added in the editor). A rename waits while the new name
    /// is empty, matches another status, or is the name another status's
    /// tasks still carry, so two statuses' tasks never merge.
    fn follow_status_renames(&mut self, si: usize, orig: &mut [Option<String>]) {
        let Some(sp) = self.data.spaces.get(si) else {
            return;
        };
        let sid = sp.id;
        let names: Vec<String> = sp.statuses.iter().map(|s| s.name.clone()).collect();
        // statuses whose new name can be passed on; a name another status's
        // tasks still carry is only free when that status moves off it in
        // the same step (a swap)
        let mut ok: Vec<bool> = (0..orig.len())
            .map(|i| match (&orig[i], names.get(i)) {
                (Some(old), Some(new)) => {
                    new != old
                        && !new.trim().is_empty()
                        && !names.iter().enumerate().any(|(j, n)| j != i && n == new)
                }
                _ => false,
            })
            .collect();
        loop {
            let mut again = false;
            for i in 0..ok.len() {
                if ok[i]
                    && orig.iter().enumerate().any(|(j, o)| {
                        j != i && !ok[j] && o.as_deref() == names.get(i).map(|s| s.as_str())
                    })
                {
                    ok[i] = false;
                    again = true;
                }
            }
            if !again {
                break;
            }
        }
        let mut map: Vec<(String, String)> = vec![];
        for (i, o) in orig.iter_mut().enumerate() {
            if ok[i] {
                map.push((o.clone().unwrap_or_default(), names[i].clone()));
                *o = Some(names[i].clone());
            }
        }
        if map.is_empty() {
            return;
        }
        let lists = self.data.lists_in(Location::Space(sid)).unwrap_or_default();
        for t in self
            .data
            .tasks
            .iter_mut()
            .filter(|t| lists.contains(&t.list_id))
        {
            if let Some((_, new)) = map.iter().find(|(old, _)| *old == t.status) {
                t.status = new.clone();
            }
        }
        self.dirty = true;
    }

    /// A status rename is done (the name field lost focus, Enter, a delete
    /// or the editor closed): tasks follow, and automations follow from the
    /// name they had when the last rename was done (`rules`) to the name
    /// now, so a name only passed through while typing (another space's
    /// status, say) never catches a workspace-wide automation.
    fn commit_status_renames(
        &mut self,
        si: usize,
        orig: &mut [Option<String>],
        rules: &mut [Option<String>],
    ) {
        self.follow_status_renames(si, orig);
        let Some(sp) = self.data.spaces.get(si) else {
            return;
        };
        let sid = sp.id;
        let mut map: Vec<(String, String)> = vec![];
        for (r, o) in rules.iter_mut().zip(orig.iter()) {
            if let (Some(old), Some(new)) = (r.as_ref(), o.as_ref()) {
                if old != new {
                    map.push((old.clone(), new.clone()));
                }
            }
            *r = o.clone();
        }
        if map.is_empty() {
            return;
        }
        let lists = self.data.lists_in(Location::Space(sid)).unwrap_or_default();
        self.retarget_automations_all(sid, &lists, &map);
        self.dirty = true;
    }

    /// The status editor's bookkeeping for space `si`: what was kept while
    /// it is open, or (just opened, or the statuses changed under it) the
    /// names the statuses have now.
    fn status_edit_lists(&self, si: usize) -> (Vec<Option<String>>, Vec<Option<String>>) {
        let sp = &self.data.spaces[si];
        if let Some((_, orig, rules)) = self
            .status_edit
            .as_ref()
            .filter(|e| e.0 == sp.id && e.1.len() == sp.statuses.len() && e.2.len() == e.1.len())
        {
            return (orig.clone(), rules.clone());
        }
        let orig: Vec<Option<String>> = sp.statuses.iter().map(|s| Some(s.name.clone())).collect();
        (orig.clone(), orig)
    }

    /// The status editor is done with its space (closed, switched to another
    /// space, or the app quits): renames reach tasks and automations, and a
    /// name left empty or taken goes back, so no task is left on a status
    /// that doesn't exist.
    fn finish_status_edit(&mut self) {
        let Some(sid) = self.edit_statuses.take() else {
            self.status_edit = None;
            return;
        };
        let kept = self.status_edit.take().filter(|e| e.0 == sid);
        let kept = kept.or_else(|| {
            let si = self.data.spaces.iter().position(|s| s.id == sid)?;
            let (o, r) = self.status_edit_lists(si);
            Some((sid, o, r))
        });
        self.settle_status_edit(kept);
    }

    /// Commits and settles one space's status edits (see `finish_status_edit`).
    fn settle_status_edit(
        &mut self,
        edit: Option<(u64, Vec<Option<String>>, Vec<Option<String>>)>,
    ) {
        let Some((sid, mut orig, mut rules)) = edit else {
            return;
        };
        let Some(si) = self.data.spaces.iter().position(|s| s.id == sid) else {
            return;
        };
        if orig.len() != self.data.spaces[si].statuses.len() || rules.len() != orig.len() {
            return;
        }
        self.commit_status_renames(si, &mut orig, &mut rules);
        if self.settle_status_names(si, &orig) {
            self.toast(tr(
                "A status name that was empty or already used went back to its last name",
            ));
        }
    }

    /// The status editor closes: a name still empty or taken goes back to
    /// the last name its tasks carry (a status added in the editor gets a
    /// free name), so no task is left on a status that doesn't exist.
    /// Returns whether a name had to be changed.
    fn settle_status_names(&mut self, si: usize, orig: &[Option<String>]) -> bool {
        let Some(sp) = self.data.spaces.get_mut(si) else {
            return false;
        };
        let mut fixed = false;
        for (st, o) in sp.statuses.iter_mut().zip(orig.iter()) {
            if let Some(o) = o {
                if st.name != *o {
                    st.name = o.clone();
                    fixed = true;
                }
            }
        }
        for i in 0..sp.statuses.len() {
            if orig.get(i).is_some_and(|o| o.is_some()) {
                continue;
            }
            let taken = |sp: &Space, n: &str| {
                sp.statuses
                    .iter()
                    .enumerate()
                    .any(|(j, s)| j != i && s.name == n)
            };
            let name = sp.statuses[i].name.trim().to_string();
            if !name.is_empty() && !taken(sp, &name) {
                continue;
            }
            let base = if name.is_empty() {
                "new status".to_string()
            } else {
                name
            };
            let mut cand = base.clone();
            let mut k = 2;
            while taken(sp, &cand) {
                cand = format!("{} {}", base, k);
                k += 1;
            }
            sp.statuses[i].name = cand;
            fixed = true;
        }
        if fixed {
            self.dirty = true;
        }
        fixed
    }

    /// Points automations on status `old` of space `sid` to `new` (see
    /// `retarget_automations_all`).
    fn retarget_automations(&mut self, sid: u64, lists: &[u64], old: &str, new: &str) {
        self.retarget_automations_all(sid, lists, &[(old.to_string(), new.to_string())]);
    }

    /// Points automations on each status `old` of space `sid` to its `new`,
    /// all at once (so swapping two names swaps their rules): those that
    /// run in the space or one of its lists always, workspace-wide ones
    /// only when no other space has a status called `old`.
    fn retarget_automations_all(&mut self, sid: u64, lists: &[u64], map: &[(String, String)]) {
        let elsewhere: Vec<bool> = map
            .iter()
            .map(|(old, _)| {
                self.data
                    .spaces
                    .iter()
                    .any(|s| s.id != sid && s.statuses.iter().any(|x| x.name == *old))
            })
            .collect();
        for a in self.data.extra.automations.iter_mut() {
            let Some(k) = map.iter().position(|(old, _)| *old == a.trigger_status) else {
                continue;
            };
            let here = match a.list {
                Some(l) => l == sid || lists.contains(&l),
                None => !elsewhere[k],
            };
            if here {
                a.trigger_status = map[k].1.clone();
            }
        }
    }

    #[cfg(test)]
    /// Moves the tasks of a status deleted in the status editor to a status of
    /// the same kind (else the first one) and notes it in each task's history.
    fn move_off_status(&mut self, si: usize, old: &str, kind: StatusKind, orig: &[Option<String>]) {
        self.move_off_status_with(si, old, old, kind, orig);
    }

    /// `move_off_status` where the automations of the deleted status are
    /// still on `rule_old` (its rename wasn't done when it was deleted).
    fn move_off_status_with(
        &mut self,
        si: usize,
        old: &str,
        rule_old: &str,
        kind: StatusKind,
        orig: &[Option<String>],
    ) {
        let Some(sp) = self.data.spaces.get(si) else {
            return;
        };
        let sid = sp.id;
        // the name tasks carry for the status they move to
        let carried = |i: usize| {
            orig.get(i)
                .cloned()
                .flatten()
                .unwrap_or_else(|| sp.statuses[i].name.clone())
        };
        let Some(to_i) =
            sp.statuses
                .iter()
                .position(|s| s.kind == kind)
                .or(if sp.statuses.is_empty() {
                    None
                } else {
                    Some(0)
                })
        else {
            return;
        };
        let to = carried(to_i);
        let shown = sp.statuses[to_i].name.clone();
        let lists = self.data.lists_in(Location::Space(sid)).unwrap_or_default();
        let me = self.data.me;
        for t in self
            .data
            .tasks
            .iter_mut()
            .filter(|t| lists.contains(&t.list_id) && t.status == old)
        {
            t.status = to.clone();
            Self::log(
                t,
                me,
                format!(
                    "changed status from {} to {}",
                    old.to_uppercase(),
                    shown.to_uppercase()
                ),
            );
        }
        // automations on the deleted status follow its tasks
        self.retarget_automations(sid, &lists, rule_old, &to);
        self.dirty = true;
    }

    /// Puts a list, with its settings, place and tasks, into Trash (the list
    /// itself is removed by the caller).
    fn trash_list(&mut self, id: u64) {
        let Some(sp) = self.data.space_of_list(id) else {
            return;
        };
        let space = sp.id;
        let folder = sp
            .folders
            .iter()
            .find(|f| f.lists.iter().any(|l| l.id == id))
            .map(|f| f.id);
        let Some(list) = self.data.list_item(id).cloned() else {
            return;
        };
        let tasks: Vec<Task> = self
            .data
            .tasks
            .iter()
            .filter(|t| t.list_id == id)
            .cloned()
            .collect();
        let title = list.name.clone();
        let statuses = sp.statuses.clone();
        let payload = serde_json::to_string(&ListTrash {
            space,
            folder,
            list,
            tasks,
            statuses,
        })
        .unwrap_or_default();
        let tid = self.data.new_id();
        self.data.extra.trash.push(TrashItem {
            id: tid,
            kind: "List".into(),
            title,
            deleted: now_ts(),
            payload,
        });
    }

    /// The id of the Trash entry made last (by `trash_list`).
    fn last_trash_id(&self) -> Option<u64> {
        self.data.extra.trash.last().map(|x| x.id)
    }

    // ---------------------------------------------------------------- top bar

    fn open_quick_add(&mut self, due: Option<NaiveDate>) {
        if let Some(list) = self.default_list() {
            let mut qa = QuickAdd {
                list,
                due,
                ..Default::default()
            };
            // made from My Tasks / Today & Overdue: mine (and due today there)
            if self.section == Section::Home
                && matches!(self.loc, Location::MyTasks | Location::TodayOverdue)
            {
                qa.assignees.push(self.data.me);
                if self.loc == Location::TodayOverdue && qa.due.is_none() {
                    qa.due = Some(today());
                }
            }
            self.quick_add = Some(qa);
        } else {
            self.need_a_list();
        }
    }

    /// No List a new task could go in: say why and open the next step, a
    /// new List (or a new Space first, which comes with a List).
    pub(super) fn need_a_list(&mut self) {
        let msg = if self.open_spaces().is_empty() {
            "Tasks live in a List. Make a Space first: it comes with one."
        } else {
            "Tasks live in a List. Make one first."
        };
        self.toast(tr(msg));
        self.offer_new_list(self.current_space());
    }

    /// The New List dialog, or New Space when there is no Space to put a
    /// List in yet (the Home Lists card does the same).
    pub(super) fn offer_new_list(&mut self, space: Option<u64>) {
        if self.open_spaces().is_empty() {
            self.modal = Some(Modal::NewSpace {
                name: String::new(),
                color: STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13],
            });
        } else {
            self.open_new_list(space, None);
        }
    }

    // ---------------------------------------------------------------- sidebar

    // ---------------------------------------------------------------- content

    fn content(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(t.app_bg)
                    .inner_margin(egui::Margin {
                        left: 0,
                        right: 6,
                        top: 0,
                        bottom: 6,
                    }),
            )
            .show(ctx, |ui| {
                let card = ui.available_rect_before_wrap();
                ui.painter().rect_filled(card, 8.0, t.bg);
                let mut inner = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(card.shrink(1.0))
                        .layout(Layout::top_down(Align::Min)),
                );
                inner.set_clip_rect(card.shrink(1.0));
                let ui = &mut inner;
                if !self.recent.is_empty() {
                    self.tray(ui);
                }
                let page = egui::Frame::new().fill(t.bg);
                match self.section {
                    Section::Home => {
                        self.sidebar(ui);
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| match self.loc {
                                Location::Home => self.home(ui),
                                Location::Inbox => self.inbox_page(ui),
                                Location::Replies => self.inbox(ui, 1),
                                Location::Assigned => self.inbox(ui, 2),
                                _ => {
                                    self.header(ui);
                                    let rect = ui.available_rect_before_wrap();
                                    ui.scope_builder(
                                        egui::UiBuilder::new()
                                            .max_rect(rect.shrink2(vec2(16.0, 0.0))),
                                        |ui| match self.view {
                                            ViewKind::Overview => self.overview_view(ui),
                                            ViewKind::List => self.list_view(ui),
                                            ViewKind::Board => self.board_view(ui),
                                            ViewKind::Calendar => self.calendar_view(ui),
                                            ViewKind::Gantt => self.gantt_view(ui),
                                            ViewKind::Timeline => self.timeline_view(ui),
                                            ViewKind::Table => self.table_view(ui),
                                        },
                                    );
                                }
                            });
                    }
                    Section::Chat => {
                        self.sidebar(ui);
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.chat(ui));
                    }
                    Section::Planner => {
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.planner(ui));
                    }
                    Section::Posts => {
                        self.sidebar(ui);
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.posts_page(ui));
                    }
                    Section::Meetings => {
                        self.sidebar(ui);
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.meetings_page(ui));
                    }
                    Section::People => {
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.people_page(ui));
                    }
                    Section::Goals => {
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.goals_hub(ui));
                    }
                    Section::Forms => {
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.forms_hub(ui));
                    }
                    Section::Whiteboards => {
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.whiteboards_hub(ui));
                    }
                    Section::Settings => {
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.settings_page(ui));
                    }
                    Section::Ai => {
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.ai_page(ui));
                    }
                    Section::Teams => {
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.teams_hub(ui));
                    }
                    Section::Docs => {
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.docs_hub(ui));
                    }
                    Section::Dashboards => {
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.dashboards_hub(ui));
                    }
                    Section::Timesheets => {
                        egui::CentralPanel::default()
                            .frame(page)
                            .show_inside(ui, |ui| self.timesheets_hub(ui));
                    }
                }
                ui.painter().rect_stroke(
                    card,
                    8.0,
                    Stroke::new(1.0_f32, t.border),
                    egui::StrokeKind::Inside,
                );
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
        let week_anchor: NaiveDate = ui
            .data(|d| d.get_temp(egui::Id::new("cal_week_anchor")))
            .unwrap_or(td);
        let mut week_anchor = week_anchor;
        ui.horizontal(|ui| {
            if ui.button(tr("Today")).clicked() {
                self.cal_month = NaiveDate::from_ymd_opt(td.year(), td.month(), 1).unwrap();
                week_anchor = td;
            }
            let (back, fwd) = icons::period_arrows(ui, &t);
            if back {
                if self.cal_week {
                    week_anchor -= Duration::days(7);
                } else {
                    self.cal_month = add_months(self.cal_month, -1);
                }
            }
            if fwd {
                if self.cal_week {
                    week_anchor += Duration::days(7);
                } else {
                    self.cal_month = add_months(self.cal_month, 1);
                }
            }
            let title = if self.cal_week {
                let ws = week_anchor
                    - Duration::days(week_anchor.weekday().num_days_from_sunday() as i64);
                week_range(ws)
            } else {
                self.cal_month.text("%B %Y")
            };
            ui.label(RichText::new(title).size(17.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.selectable_label(!self.cal_week, tr("Month")).clicked() {
                    self.cal_week = false;
                    self.cal_month =
                        NaiveDate::from_ymd_opt(week_anchor.year(), week_anchor.month(), 1)
                            .unwrap();
                }
                if ui.selectable_label(self.cal_week, tr("Week")).clicked() {
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
        let ch = if self.cal_week {
            (avail.height() - head - 10.0).max(200.0)
        } else {
            ((avail.height() - head - 34.0) / 6.0).max(96.0)
        };
        let area = Rect::from_min_size(avail.min, vec2(avail.width(), head + ch * rows as f32));
        ui.allocate_rect(full, Sense::hover());
        let p = ui.painter().clone();
        for (i, d) in ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
            .iter()
            .enumerate()
        {
            p.text(
                pos2(area.left() + cw * i as f32 + 8.0, area.top() + head / 2.0),
                Align2::LEFT_CENTER,
                tr(d),
                FontId::proportional(12.0),
                t.text3,
            );
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
        // bar labels: (room, x, text, font, colour), drawn over all the days
        let mut labels: Vec<(Rect, f32, String, FontId, Color32)> = vec![];
        // a task with a start and a due date shows on every day between
        let span = |x: &Task| match (x.start, x.due) {
            (Some(s), Some(e)) if s <= e => Some((s, e)),
            (_, Some(e)) => Some((e, e)),
            _ => None,
        };
        // the time a bar starts at: its start time, or a one-day task's due time
        let bar_time = |x: &Task, s: NaiveDate, e: NaiveDate| match x.start_at() {
            Some(at) if x.start == Some(s) => Some(at),
            _ if s == e => x.due_at(),
            _ => None,
        };
        // each week row gives every bar one line for all its days, so a bar
        // that runs over several days stays in one piece: earlier starts and
        // longer bars take the top lines, then all-day before timed ones
        let mut lanes: HashMap<(i64, u64), usize> = HashMap::new();
        for w in 0..rows as i64 {
            let ws = start + Duration::days(w * 7);
            let we = ws + Duration::days(6);
            let mut week: Vec<(&Task, NaiveDate, NaiveDate, NaiveDate, NaiveDate)> = tasks
                .iter()
                .filter_map(|x| {
                    span(x)
                        .filter(|(s, e)| *s <= we && *e >= ws)
                        .map(|(s, e)| (x, s, e, s.max(ws), e.min(we)))
                })
                .collect();
            week.sort_by_key(|(x, s, e, a, b)| {
                let at = bar_time(x, *s, *e);
                (
                    *a,
                    std::cmp::Reverse((*b - *a).num_days()),
                    at.is_some(),
                    at,
                    x.id,
                )
            });
            // the last day each line is taken up to
            let mut until: Vec<NaiveDate> = vec![];
            for (x, _, _, a, b) in week {
                let lane = match until.iter().position(|u| *u < a) {
                    Some(l) => l,
                    None => {
                        until.push(b);
                        until.len() - 1
                    }
                };
                until[lane] = b;
                lanes.insert((w, x.id), lane);
            }
        }
        for i in 0..(rows * 7) as i64 {
            let d = start + Duration::days(i);
            let cell = Rect::from_min_size(
                pos2(
                    area.left() + cw * (i % 7) as f32,
                    area.top() + head + ch * (i / 7) as f32,
                ),
                vec2(cw, ch),
            );
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
            p.rect_stroke(
                cell,
                0.0,
                Stroke::new(0.5_f32, t.border),
                egui::StrokeKind::Inside,
            );
            let num_pos = cell.left_top() + vec2(10.0, 12.0);
            if d == td {
                p.circle_filled(num_pos + vec2(4.0, 0.0), 11.0, t.danger);
                p.text(
                    num_pos + vec2(4.0, 0.0),
                    Align2::CENTER_CENTER,
                    d.day().to_string(),
                    FontId::proportional(12.0),
                    Color32::WHITE,
                );
            } else {
                p.text(
                    num_pos,
                    Align2::LEFT_CENTER,
                    d.day().to_string(),
                    FontId::proportional(12.0),
                    if in_month { t.text } else { t.text3 },
                );
            }
            if hov && !dragging {
                let br =
                    Rect::from_center_size(cell.right_top() + vec2(-14.0, 12.0), vec2(20.0, 20.0));
                let b = ui.interact(br, ui.id().with(("caladd", i)), Sense::click());
                p.rect_filled(br, 4.0, if b.hovered() { t.selected_bg } else { t.hover });
                p.text(
                    br.center(),
                    Align2::CENTER_CENTER,
                    "+",
                    FontId::proportional(14.0),
                    t.text2,
                );
                if b.clicked() {
                    self.open_quick_add(Some(d));
                }
            }
            let week_row = i / 7;
            let mut day_tasks: Vec<(&Task, NaiveDate, NaiveDate, usize)> = tasks
                .iter()
                .filter_map(|x| {
                    let lane = *lanes.get(&(week_row, x.id))?;
                    span(x)
                        .filter(|(s, e)| *s <= d && d <= *e)
                        .map(|(s, e)| (x, s, e, lane))
                })
                .collect();
            day_tasks.sort_by_key(|x| x.3);
            // lines of 22 that fit under the day number; when some don't, the
            // last line that fits says "+N more" instead
            let fit = ((ch - 24.0) / 22.0).floor().max(1.0) as usize;
            let needed = day_tasks.last().map(|x| x.3 + 1).unwrap_or(0);
            let max = if needed > fit { fit - 1 } else { fit };
            for (task, s0, e0, k) in day_tasks.iter().copied() {
                if k >= max {
                    let hidden = day_tasks.iter().filter(|x| x.3 >= max).count();
                    let more = Rect::from_min_size(
                        cell.left_top() + vec2(4.0, 26.0 + 22.0 * max as f32),
                        vec2(cw - 8.0, 20.0),
                    );
                    let mr = ui.interact(more, ui.id().with(("calmore", i)), Sense::click());
                    name_it(&mr, &format!("More tasks on {}", d));
                    if mr.hovered() {
                        p.rect_filled(more, 4.0, t.hover);
                    }
                    p.text(
                        more.left_center() + vec2(4.0, 0.0),
                        Align2::LEFT_CENTER,
                        trf!("+{} more", hidden),
                        FontId::proportional(11.5),
                        if mr.hovered() { t.text } else { t.text3 },
                    );
                    // the day's whole list in a popup
                    let all: Vec<(u64, String, Status)> = day_tasks
                        .iter()
                        .map(|(x, _, _, _)| (x.id, x.name.clone(), self.data.status_of(x)))
                        .collect();
                    popup(ui, &mr, ui.id().with(("calmore_pop", i)), 240.0, |ui| {
                        ui.label(RichText::new(d.text("%A, %b %-d")).strong());
                        ui.add_space(4.0);
                        for (tid, name, st) in &all {
                            let r = menu_row(ui, &t, name, false, |p, c| {
                                paint_status_circle(p, c, 5.5, st)
                            });
                            if r.clicked() {
                                self.actions.push(Action::Open(*tid));
                                close_popup(ui);
                            }
                        }
                    });
                    break;
                }
                let starts = d == s0 || i % 7 == 0;
                let goes_on = d < e0 && i % 7 != 6;
                let left = if d > s0 && i % 7 != 0 {
                    cell.left()
                } else {
                    cell.left() + 4.0
                };
                let right = if goes_on {
                    cell.right()
                } else {
                    cell.right() - 4.0
                };
                let r = Rect::from_min_max(
                    pos2(left, cell.top() + 26.0 + 22.0 * k as f32),
                    pos2(right, cell.top() + 46.0 + 22.0 * k as f32),
                );
                let resp = ui.interact(
                    r,
                    ui.id().with(("calt", task.id, i)),
                    Sense::click_and_drag(),
                );
                if resp.drag_started() {
                    egui::DragAndDrop::set_payload(ui.ctx(), task.id);
                    // remember the day it was picked up by, to move a span as a whole
                    ui.data_mut(|m| m.insert_temp(egui::Id::new("cal_grab"), d));
                }
                let st = self.data.status_of(task);
                let col = rgb(st.color);
                let round = egui::CornerRadius {
                    nw: if d > s0 && i % 7 != 0 { 0 } else { 4 },
                    sw: if d > s0 && i % 7 != 0 { 0 } else { 4 },
                    ne: if goes_on { 0 } else { 4 },
                    se: if goes_on { 0 } else { 4 },
                };
                p.rect_filled(
                    r,
                    round,
                    if resp.hovered() {
                        col.gamma_multiply(0.30)
                    } else {
                        col.gamma_multiply(0.16)
                    },
                );
                if starts {
                    p.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 2.0, col);
                    // the label may use the bar's whole length in this week; it
                    // is drawn after all days so later days don't paint over it
                    let week_end = start + Duration::days(week_row * 7 + 6);
                    let days = (e0.min(week_end) - d).num_days() as f32;
                    let room = Rect::from_min_max(r.min, pos2(r.right() + cw * days, r.bottom()));
                    let mut label_x = r.left() + 8.0;
                    if let Some(at) = bar_time(task, s0, e0).filter(|_| d == s0) {
                        let font = FontId::proportional(11.0);
                        let w = p
                            .layout_no_wrap(fmt_time(at), font.clone(), t.text2)
                            .size()
                            .x;
                        labels.push((room, label_x, fmt_time(at), font, t.text2));
                        label_x += w + 4.0;
                    }
                    let font = FontId::proportional(12.0);
                    let name = fit_text(&p, &task.name, &font, room.right() - 4.0 - label_x);
                    labels.push((room, label_x, name, font, t.text));
                }
                name_it(&resp, &format!("Calendar {} {}", task.name, d));
                let tid = task.id;
                resp.context_menu(|ui| self.task_menu(ui, tid));
                if resp.clicked() {
                    self.actions.push(Action::Open(task.id));
                }
            }
        }
        for (room, x, text, font, color) in labels {
            p.with_clip_rect(room).text(
                pos2(x, room.center().y),
                Align2::LEFT_CENTER,
                text,
                font,
                color,
            );
        }
        // Unscheduled / Overdue panel
        p.rect_filled(side, 8.0, t.column_bg);
        let mut side_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(side.shrink(10.0))
                .layout(Layout::top_down(Align::Min)),
        );
        let tab: u8 = side_ui
            .data(|d| d.get_temp(egui::Id::new("cal_side_tab")))
            .unwrap_or(0);
        let unscheduled: Vec<&Task> = tasks.iter().filter(|x| x.due.is_none()).collect();
        let now = now_local();
        let overdue: Vec<&Task> = tasks
            .iter()
            .filter(|x| x.is_overdue_at(now) && !self.data.is_closed(x))
            .collect();
        let mut new_tab = tab;
        side_ui.horizontal(|ui| {
            if ui
                .selectable_label(tab == 0, trf!("{} Unscheduled", unscheduled.len()))
                .clicked()
            {
                new_tab = 0;
            }
            if ui
                .selectable_label(tab == 1, trf!("{} Overdue", overdue.len()))
                .clicked()
            {
                new_tab = 1;
            }
        });
        side_ui.data_mut(|d| d.insert_temp(egui::Id::new("cal_side_tab"), new_tab));
        side_ui.label(
            RichText::new(tr("Drag a task onto a day to schedule it."))
                .small()
                .color(t.text3),
        );
        side_ui.add_space(4.0);
        let list = if tab == 0 { unscheduled } else { overdue };
        egui::ScrollArea::vertical()
            .id_salt("cal_side")
            .auto_shrink([false, false])
            .show(&mut side_ui, |ui| {
                for task in list {
                    let w = ui.available_width();
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click_and_drag());
                    if resp.drag_started() {
                        egui::DragAndDrop::set_payload(ui.ctx(), task.id);
                        ui.data_mut(|m| m.remove::<NaiveDate>(egui::Id::new("cal_grab")));
                    }
                    ui.painter()
                        .rect_filled(r, 6.0, if resp.hovered() { t.hover } else { t.card });
                    ui.painter().rect_stroke(
                        r,
                        6.0,
                        Stroke::new(1.0_f32, t.border),
                        egui::StrokeKind::Inside,
                    );
                    let st = self.data.status_of(task);
                    paint_status_circle(ui.painter(), r.left_center() + vec2(14.0, 0.0), 6.0, &st);
                    ui.painter().with_clip_rect(r.shrink(2.0)).text(
                        r.left_center() + vec2(28.0, 0.0),
                        Align2::LEFT_CENTER,
                        &task.name,
                        FontId::proportional(12.5),
                        t.text,
                    );
                    name_it(&resp, &format!("Unscheduled {}", task.name));
                    let tid = task.id;
                    resp.context_menu(|ui| self.task_menu(ui, tid));
                    if resp.clicked() {
                        self.actions.push(Action::Open(task.id));
                    }
                    ui.add_space(4.0);
                }
            });
        if dragging {
            if let Some(pos) = ui.ctx().pointer_hover_pos() {
                egui::Area::new(egui::Id::new("cal_drag_ghost"))
                    .order(egui::Order::Tooltip)
                    .fixed_pos(pos + vec2(10.0, 6.0))
                    .show(ui.ctx(), |ui| {
                        icons::icon_label(
                            ui,
                            "calendar",
                            RichText::new(tr("Drop on a day")).small(),
                            t.text2,
                        );
                    });
            }
        }
        if let Some(d) = drop_on {
            if let Some(id) = egui::DragAndDrop::take_payload::<u64>(ui.ctx()) {
                let grab: Option<NaiveDate> =
                    ui.data_mut(|m| m.remove_temp(egui::Id::new("cal_grab")));
                if let Some(mut task) = self.task(*id).cloned() {
                    match (grab, task.start, task.due) {
                        // a task picked up from the grid moves by as many days as it was dragged
                        (Some(g), s, Some(e)) => {
                            let delta = d - g;
                            task.due = Some(e + delta);
                            task.start = s.map(|s| s + delta);
                        }
                        _ => {
                            task.due = Some(d);
                            if task.start.map_or(false, |s| s > d) {
                                task.start = Some(d);
                            }
                        }
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
        let sc = timebar::DayScale::new(self.gantt_zoom, td);
        let (start, cw) = (sc.start, sc.cw);
        let rh = 34.0;
        let name_w = 260.0;
        let head = 44.0;
        ui.horizontal(|ui| {
            if timebar::zoom_buttons(ui, &mut self.gantt_zoom) {
                self.gantt_jump = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button(tr("Export CSV"))
                    .on_hover_text(tr("Save the tasks in this view as a CSV file"))
                    .clicked()
                {
                    self.toast(self.export_csv());
                }
                let undated = tasks
                    .iter()
                    .filter(|x| x.start.is_none() && x.due.is_none())
                    .count();
                if undated > 0 {
                    ui.label(
                        RichText::new(trf!("{} without dates", undated))
                            .small()
                            .color(t.text3),
                    );
                }
            });
        });
        ui.add_space(4.0);
        // each task's subtasks follow it once it is expanded
        let mut rows: Vec<(Task, usize, usize)> = vec![];
        for task in &tasks {
            let subs: Vec<Task> = self
                .subtasks(task.id)
                .into_iter()
                .filter(|x| !x.archived)
                .cloned()
                .collect();
            rows.push((task.clone(), 0, subs.len()));
            if self.expanded_tasks.contains(&task.id) {
                rows.extend(subs.into_iter().map(|x| (x, 1, 0)));
            }
        }
        // fill the view so the horizontal scrollbar sits at the bottom, not under the last row
        let rows_h = (head + rh * (rows.len() + 1) as f32).max(ui.available_height() - 24.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    // names
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.set_width(name_w);
                        let (hr, _) = ui.allocate_exact_size(vec2(name_w, head), Sense::hover());
                        ui.painter().text(
                            hr.left_center() + vec2(8.0, 0.0),
                            Align2::LEFT_CENTER,
                            tr("Name"),
                            FontId::proportional(12.0),
                            t.text3,
                        );
                        ui.painter()
                            .hline(hr.x_range(), hr.bottom(), Stroke::new(1.0_f32, t.border));
                        for (task, depth, nsub) in &rows {
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(name_w, rh), Sense::click());
                            if resp.hovered() {
                                ui.painter().rect_filled(r, 0.0, t.hover);
                            }
                            let x0 = r.left() + 16.0 + *depth as f32 * 20.0;
                            // the arrow that shows or hides the subtasks
                            let ar = Rect::from_center_size(
                                pos2(x0 - 4.0, r.center().y),
                                vec2(16.0, 16.0),
                            );
                            let mut on_arrow = false;
                            if *nsub > 0 {
                                let a = ui.interact(
                                    ar,
                                    ui.id().with(("gexp", task.id)),
                                    Sense::click(),
                                );
                                name_it(&a, &format!("Subtasks of {}", task.name));
                                let open = self.expanded_tasks.contains(&task.id);
                                paint_triangle(
                                    ui.painter(),
                                    ar.center(),
                                    open,
                                    if a.hovered() { t.text } else { t.text3 },
                                );
                                if a.clicked() {
                                    if open {
                                        self.expanded_tasks.remove(&task.id);
                                    } else {
                                        self.expanded_tasks.insert(task.id);
                                    }
                                }
                                on_arrow = a.clicked() || a.hovered();
                            }
                            let st = self.data.status_of(task);
                            paint_status_circle(
                                ui.painter(),
                                pos2(x0 + 12.0, r.center().y),
                                6.5,
                                &st,
                            );
                            // the name shortened to leave room for the subtask count
                            // a subtasks icon and the count
                            let badge = nsub.to_string();
                            let badge_w = if *nsub > 0 {
                                ui.painter()
                                    .layout_no_wrap(
                                        badge.clone(),
                                        FontId::proportional(11.5),
                                        t.text3,
                                    )
                                    .size()
                                    .x
                                    + 6.0
                                    + 16.0
                            } else {
                                0.0
                            };
                            let nr = paint_fit(
                                ui,
                                pos2(x0 + 26.0, r.center().y),
                                Align2::LEFT_CENTER,
                                &task.name,
                                FontId::proportional(13.0),
                                t.text,
                                r.right() - 6.0 - badge_w - (x0 + 26.0),
                            );
                            if *nsub > 0 {
                                icons::line_icon_scaled(
                                    ui.painter(),
                                    "subtasks",
                                    pos2(nr.right() + 12.0, r.center().y),
                                    t.text3,
                                    0.75,
                                );
                                ui.painter().text(
                                    pos2(nr.right() + 21.0, r.center().y),
                                    Align2::LEFT_CENTER,
                                    badge,
                                    FontId::proportional(11.5),
                                    t.text3,
                                );
                            }
                            ui.painter()
                                .hline(r.x_range(), r.bottom(), Stroke::new(0.5_f32, t.border));
                            let tid = task.id;
                            resp.context_menu(|ui| self.task_menu(ui, tid));
                            if resp.clicked() && !on_arrow {
                                self.actions.push(Action::Open(task.id));
                            }
                        }
                        let (r, resp) = ui.allocate_exact_size(vec2(name_w, rh), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 0.0, t.hover);
                        }
                        ui.painter().text(
                            r.left_center() + vec2(10.0, 0.0),
                            Align2::LEFT_CENTER,
                            tr("+ Add task"),
                            FontId::proportional(13.0),
                            t.text3,
                        );
                        if resp.clicked() {
                            self.open_quick_add(None);
                        }
                        if rows.is_empty() {
                            ui.add_space(12.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(tr(
                                        "No tasks here yet. Add one, then click or drag across the days in its row to give it dates.",
                                    ))
                                    .small()
                                    .color(t.text3),
                                )
                                .wrap(),
                            );
                        }
                    });
                    // the divider only runs as far as the rows (it used to run to the bottom)
                    let vr = ui.available_rect_before_wrap();
                    ui.painter().vline(
                        vr.left(),
                        vr.top()..=vr.top() + rows_h,
                        Stroke::new(1.0_f32, t.border),
                    );
                    let jump = std::mem::take(&mut self.gantt_jump);
                    let mut sa = egui::ScrollArea::horizontal()
                        .id_salt(("gantt_h", self.gantt_zoom))
                        .auto_shrink([false, true]);
                    if jump {
                        sa = sa.horizontal_scroll_offset(sc.today_offset(td, vr.width()));
                    }
                    sa.show(ui, |ui| {
                        let total_h = rows_h;
                        let (area, _) =
                            ui.allocate_exact_size(vec2(sc.width(), total_h), Sense::hover());
                        let p = ui.painter().clone();
                        timebar::paint_axis(&p, &t, area, head, &sc, td);
                        for (k, (task, _, _)) in rows.iter().enumerate() {
                            let y = area.top() + head + rh * k as f32;
                            p.hline(area.x_range(), y + rh, Stroke::new(0.5_f32, t.border));
                            let day_at = |x: f32| sc.day_at(area.left(), x);
                            let Some((s, e)) = timebar::bar_range(task.start, task.due) else {
                                // no dates yet: click a day, or drag across days, to schedule it
                                let row = Rect::from_min_size(
                                    pos2(area.left(), y),
                                    vec2(area.width(), rh),
                                );
                                let resp = ui.interact(
                                    row,
                                    ui.id().with(("gsched", task.id)),
                                    Sense::click_and_drag(),
                                );
                                name_it(&resp, &format!("Schedule {}", task.name));
                                let from_id = resp.id.with("from");
                                if resp.drag_started() {
                                    if let Some(pp) = resp.interact_pointer_pos() {
                                        ui.data_mut(|d| d.insert_temp(from_id, day_at(pp.x)));
                                    }
                                }
                                let from: Option<NaiveDate> = ui.data(|d| d.get_temp(from_id));
                                let at = resp
                                    .interact_pointer_pos()
                                    .or(resp.hover_pos())
                                    .map(|pp| day_at(pp.x));
                                if let Some(at) = at {
                                    let (a0, a1) =
                                        match (resp.dragged() || resp.drag_stopped(), from) {
                                            (true, Some(f)) => (f.min(at), f.max(at)),
                                            _ => (at, at),
                                        };
                                    let gx0 =
                                        area.left() + cw * (a0 - start).num_days() as f32 + 1.0;
                                    let gx1 = area.left()
                                        + cw * ((a1 - start).num_days() + 1) as f32
                                        - 1.0;
                                    let ghost = Rect::from_min_max(
                                        pos2(gx0, y + 7.0),
                                        pos2(gx1.max(gx0 + 8.0), y + rh - 7.0),
                                    );
                                    p.rect_stroke(
                                        ghost,
                                        5.0,
                                        Stroke::new(1.0_f32, t.text3),
                                        egui::StrokeKind::Inside,
                                    );
                                    if resp.clicked() || resp.drag_stopped() {
                                        let mut e2 = task.clone();
                                        e2.start = if a0 < a1 { Some(a0) } else { None };
                                        e2.due = Some(a1);
                                        self.actions.push(Action::Replace(e2));
                                    }
                                }
                                if resp.drag_stopped() {
                                    ui.data_mut(|d| d.remove::<NaiveDate>(from_id));
                                }
                                continue;
                            };
                            self.date_bar(ui, &p, &sc, area.left(), y, rh, task, (s, e), "gantt");
                        }
                    });
                });
            });
    }

    // ---------------------------------------------------------------- table view

    fn table_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        let mut tasks: Vec<Task> = self.filtered().into_iter().cloned().collect();
        let now = now_ts();
        // the columns: the fixed ones and one per custom field; all but # and
        // Name can be hidden with the "+" at the end of the header
        let fields: Vec<CustomField> = if self.data.extra.apps.custom_fields {
            self.data.extra.fields.clone()
        } else {
            vec![]
        };
        let mut cols: Vec<TableCol> = vec![TableCol::Num, TableCol::Name];
        // columns of ClickApps that are off take no room
        let apps = self.data.extra.apps.clone();
        let app_off = |c: TableCol| match c {
            TableCol::Priority => !apps.priorities,
            TableCol::Tags => !apps.tags,
            TableCol::Tracked => !apps.time_tracking,
            _ => false,
        };
        for c in TableCol::FIXED {
            if !self.hidden_cols.contains(&c.key()) && !app_off(c) {
                cols.push(c);
            }
        }
        for f in &fields {
            let c = TableCol::Field(f.id);
            if !self.hidden_cols.contains(&c.key()) {
                cols.push(c);
            }
        }
        let field_name = |id: u64| {
            fields
                .iter()
                .find(|f| f.id == id)
                .map(|f| f.name.clone())
                .unwrap_or_default()
        };
        let field_num = |id: u64| fields.iter().any(|f| f.id == id && f.kind == "number");
        // a click on a header sorts by it; another click turns the order round
        let sort_id = egui::Id::new("table_sort");
        let sort: Option<(TableCol, bool)> = ui.data(|d| d.get_temp(sort_id)).flatten();
        if let Some((col, desc)) = sort {
            let data = &self.data;
            let first_name = |x: &Task| {
                x.assignees
                    .first()
                    .and_then(|a| data.member(*a))
                    .map(|m| m.name.to_lowercase())
            };
            tasks.sort_by(|a, b| {
                use std::cmp::Ordering;
                // empty cells stay at the bottom either way
                let last = |x: Option<Ordering>| x.unwrap_or(Ordering::Equal);
                let o = match col {
                    TableCol::Num | TableCol::Created => Some(a.created.cmp(&b.created)),
                    TableCol::Name => Some(a.name.to_lowercase().cmp(&b.name.to_lowercase())),
                    TableCol::Status => {
                        let sts = data.statuses_for_list(a.list_id);
                        let pos = |x: &Task| {
                            sts.iter()
                                .position(|s| s.name == x.status)
                                .unwrap_or(usize::MAX)
                        };
                        Some(pos(a).cmp(&pos(b)))
                    }
                    TableCol::Assignee => match (first_name(a), first_name(b)) {
                        (Some(x), Some(y)) => Some(x.cmp(&y)),
                        (x, y) => return x.is_none().cmp(&y.is_none()),
                    },
                    TableCol::Due => match (a.due, b.due) {
                        (Some(x), Some(y)) => Some(x.cmp(&y)),
                        (x, y) => return x.is_none().cmp(&y.is_none()),
                    },
                    TableCol::Priority => match (a.priority, b.priority) {
                        (0, 0) => Some(Ordering::Equal),
                        (0, _) | (_, 0) => return (a.priority == 0).cmp(&(b.priority == 0)),
                        (x, y) => Some(x.cmp(&y)),
                    },
                    TableCol::Tags => match (a.tags.first(), b.tags.first()) {
                        (Some(x), Some(y)) => Some(x.to_lowercase().cmp(&y.to_lowercase())),
                        (x, y) => return x.is_none().cmp(&y.is_none()),
                    },
                    TableCol::Tracked => Some(a.tracked_secs(now).cmp(&b.tracked_secs(now))),
                    TableCol::Field(id) => {
                        let va = a.custom.get(&id).cloned().unwrap_or_default();
                        let vb = b.custom.get(&id).cloned().unwrap_or_default();
                        match (va.is_empty(), vb.is_empty()) {
                            (false, false) if field_num(id) => va
                                .parse::<f64>()
                                .unwrap_or(0.0)
                                .partial_cmp(&vb.parse::<f64>().unwrap_or(0.0)),
                            (false, false) => Some(va.to_lowercase().cmp(&vb.to_lowercase())),
                            (x, y) => return x.cmp(&y),
                        }
                    }
                };
                let o = last(o);
                if desc {
                    o.reverse()
                } else {
                    o
                }
            });
        }
        let mut widths: Vec<f32> = cols.iter().map(|c| c.width()).collect();
        // Name takes the room the other columns leave (at least 200), so the
        // last column and the "+" fit instead of running off the right edge
        if let Some(ni) = cols.iter().position(|c| *c == TableCol::Name) {
            let others: f32 = widths.iter().sum::<f32>() - widths[ni];
            widths[ni] = (ui.available_width() - others - 36.0 - 12.0).max(200.0);
        }
        let edit_id = egui::Id::new("table_rename");
        let mut renaming: Option<(u64, String)> = ui.data(|d| d.get_temp(edit_id));
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let total: f32 = widths.iter().sum::<f32>() + 36.0;
                let (hr, _) = ui.allocate_exact_size(vec2(total, 30.0), Sense::hover());
                let mut x = hr.left();
                let mut new_sort = sort;
                for (i, c) in cols.iter().enumerate() {
                    let cell = Rect::from_min_size(pos2(x, hr.top()), vec2(widths[i], hr.height()));
                    let label = match c {
                        TableCol::Field(id) => field_name(*id),
                        _ => c.label().to_string(),
                    };
                    let hresp = ui.interact(cell, ui.id().with(("thead", i)), Sense::click());
                    name_it(&hresp, &format!("Sort by {}", label));
                    if hresp.hovered() {
                        ui.painter().rect_filled(cell, 0.0, t.hover);
                    }
                    let shown = if matches!(c, TableCol::Field(_)) {
                        label
                    } else {
                        tr(&label).to_string()
                    };
                    let g = ui
                        .painter()
                        .layout_no_wrap(shown, FontId::proportional(12.0), t.text3);
                    let gw = g.size().x;
                    ui.painter().with_clip_rect(cell).galley(
                        pos2(x + 8.0, hr.center().y - g.size().y / 2.0),
                        g,
                        t.text3,
                    );
                    if let Some((sc, desc)) = sort {
                        if sc == *c {
                            ui.painter().text(
                                pos2(x + 12.0 + gw, hr.center().y),
                                Align2::LEFT_CENTER,
                                if desc { "\u{2193}" } else { "\u{2191}" },
                                FontId::proportional(12.0),
                                t.text,
                            );
                        }
                    }
                    if hresp.clicked() {
                        new_sort = match sort {
                            Some((sc, false)) if sc == *c => Some((*c, true)),
                            Some((sc, true)) if sc == *c => None,
                            _ => Some((*c, false)),
                        };
                    }
                    ui.painter()
                        .vline(x, hr.y_range(), Stroke::new(0.5_f32, t.border));
                    x += widths[i];
                }
                // "+" at the end: choose the columns
                let plus = Rect::from_min_size(pos2(x, hr.top()), vec2(36.0, hr.height()));
                let pr = ui.interact(plus, ui.id().with("thead_plus"), Sense::click());
                name_it(&pr, "Table columns");
                ui.painter()
                    .vline(x, hr.y_range(), Stroke::new(0.5_f32, t.border));
                ui.painter().text(
                    plus.center(),
                    Align2::CENTER_CENTER,
                    "+",
                    FontId::proportional(15.0),
                    if pr.hovered() { t.text } else { t.text3 },
                );
                let mut choices: Vec<(String, String, String)> = TableCol::FIXED
                    .iter()
                    .map(|c| (c.key(), c.label().to_string(), tr(c.label()).to_string()))
                    .collect();
                choices.extend(
                    fields
                        .iter()
                        .map(|f| (TableCol::Field(f.id).key(), f.name.clone(), f.name.clone())),
                );
                popup(ui, &pr, ui.id().with("thead_plus_pop"), 220.0, |ui| {
                    ui.label(RichText::new(tr("Columns")).small().color(t.text3));
                    for (key, label, shown) in &choices {
                        let mut on = !self.hidden_cols.contains(key);
                        let r = check(ui, &mut on, shown.as_str());
                        name_it(&r, &format!("Column {}", label));
                        if r.changed() {
                            if on {
                                self.hidden_cols.remove(key);
                            } else {
                                self.hidden_cols.insert(key.clone());
                            }
                        }
                    }
                    if fields.is_empty() {
                        ui.label(
                            RichText::new(tr("Custom Fields you add show here too."))
                                .small()
                                .color(t.text3),
                        );
                    }
                });
                if new_sort != sort {
                    ui.data_mut(|d| d.insert_temp(sort_id, new_sort));
                }
                ui.painter().rect_filled(
                    Rect::from_min_size(hr.left_bottom() - vec2(0.0, 1.0), vec2(total, 1.0)),
                    0.0,
                    t.border,
                );
                for (n, task) in tasks.iter().enumerate() {
                    let (rr, resp) = ui.allocate_exact_size(vec2(total, 34.0), Sense::click());
                    let id = ui.id().with(("trow", task.id));
                    if self.selected.contains(&task.id) {
                        ui.painter().rect_filled(rr, 0.0, t.selected_bg);
                    }
                    if resp.hovered() {
                        ui.painter().rect_filled(rr, 0.0, t.hover);
                    }
                    ui.painter()
                        .hline(rr.x_range(), rr.bottom(), Stroke::new(0.5_f32, t.border));
                    let mut cells = vec![];
                    let mut x = rr.left();
                    for w in &widths {
                        cells.push(Rect::from_min_size(
                            pos2(x, rr.top()),
                            vec2(*w, rr.height()),
                        ));
                        ui.painter()
                            .vline(x, rr.y_range(), Stroke::new(0.5_f32, t.border));
                        x += w;
                    }
                    let st = self.data.status_of(task);
                    let closed = matches!(st.kind, StatusKind::Closed | StatusKind::Done);
                    let mut edited = task.clone();
                    let mut changed = false;
                    let mut on_cell = false;
                    for (ci, c) in cols.iter().enumerate() {
                        let cell = cells[ci];
                        match c {
                            TableCol::Num => {
                                // row number, or the selection box on hover
                                let on = self.selected.contains(&task.id);
                                if resp.hovered()
                                    || on
                                    || !self.selected.is_empty()
                                    || ui.rect_contains_pointer(cell)
                                {
                                    // row_checkbox centres the box 2px right of the rect's left edge
                                    let bx = Rect::from_min_size(
                                        pos2(cell.center().x - 2.0, cell.top()),
                                        vec2(1.0, cell.height()),
                                    );
                                    self.row_checkbox(ui, bx, task.id, true);
                                } else {
                                    ui.painter().text(
                                        cell.center(),
                                        Align2::CENTER_CENTER,
                                        (n + 1).to_string(),
                                        FontId::proportional(12.0),
                                        t.text3,
                                    );
                                }
                            }
                            TableCol::Name => {
                                let editing =
                                    renaming.as_ref().is_some_and(|(rid, _)| *rid == task.id);
                                if editing {
                                    let mut text = renaming
                                        .as_ref()
                                        .map(|(_, s)| s.clone())
                                        .unwrap_or_default();
                                    let r = put_over(
                                        ui,
                                        cell.shrink2(vec2(4.0, 5.0)),
                                        // the cell is shorter than a 28 pt field
                                        egui::TextEdit::singleline(&mut text)
                                            .margin(egui::Margin::symmetric(6, 3))
                                            .vertical_align(egui::Align::Center)
                                            .hint_text(tr("Task name")),
                                    );
                                    name_field(&r, &format!("Name of {}", task.name));
                                    focus_once(ui, &r);
                                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        renaming = None;
                                    } else if r.lost_focus() {
                                        if !text.trim().is_empty() && text.trim() != task.name {
                                            edited.name = text.trim().to_string();
                                            changed = true;
                                        }
                                        renaming = None;
                                    } else {
                                        renaming = Some((task.id, text));
                                    }
                                    on_cell = true;
                                } else {
                                    // shortened to leave room for the pencil
                                    paint_fit(
                                        ui,
                                        cell.left_center() + vec2(8.0, 0.0),
                                        Align2::LEFT_CENTER,
                                        &task.name,
                                        FontId::proportional(13.0),
                                        t.text,
                                        cell.width() - 8.0 - 32.0,
                                    );
                                    // a pencil on hover renames in place
                                    let pen = Rect::from_center_size(
                                        cell.right_center() - vec2(16.0, 0.0),
                                        vec2(22.0, 22.0),
                                    );
                                    let pr = ui.interact(pen, id.with("ren"), Sense::click());
                                    name_it(&pr, &format!("Rename {}", task.name));
                                    if resp.hovered() || pr.hovered() {
                                        ui.painter().rect_filled(
                                            pen,
                                            5.0,
                                            if pr.hovered() { t.border } else { t.card },
                                        );
                                        icons::line_icon_scaled(
                                            ui.painter(),
                                            "pen",
                                            pen.center(),
                                            t.text2,
                                            0.8,
                                        );
                                    }
                                    if pr.clicked() {
                                        renaming = Some((task.id, task.name.clone()));
                                    }
                                    on_cell |= pr.hovered() || pr.clicked();
                                }
                            }
                            TableCol::Status => {
                                let sr = ui.interact(cell, id.with("s"), Sense::click());
                                hover_bg(ui, &sr, cell.shrink(2.0), 4.0);
                                let mut child = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(cell.shrink2(vec2(8.0, 6.0)))
                                        .layout(Layout::left_to_right(Align::Center)),
                                );
                                status_pill(&mut child, &st, Sense::hover());
                                let statuses = self.data.statuses_for_list(task.list_id);
                                if let Some(true) = popup(ui, &sr, id.with("sp"), 200.0, |ui| {
                                    status_menu(ui, &t, &statuses, &mut edited.status)
                                }) {
                                    changed = true;
                                }
                            }
                            TableCol::Assignee => {
                                let ar = ui.interact(cell, id.with("a"), Sense::click());
                                hover_bg(ui, &ar, cell.shrink(2.0), 4.0);
                                paint_avatars(
                                    ui.painter(),
                                    cell.left_center() + vec2(8.0, 0.0),
                                    10.0,
                                    &task.assignees,
                                    &self.data,
                                    t.bg,
                                );
                                let data = &self.data;
                                if let Some(true) = popup(ui, &ar, id.with("ap"), 220.0, |ui| {
                                    assignee_menu(ui, &t, data, &mut edited.assignees)
                                }) {
                                    changed = true;
                                }
                            }
                            TableCol::Due => {
                                let dr = ui.interact(cell, id.with("d"), Sense::click());
                                hover_bg(ui, &dr, cell.shrink(2.0), 4.0);
                                if let Some(d) = fmt_due(task) {
                                    ui.painter().text(
                                        cell.left_center() + vec2(8.0, 0.0),
                                        Align2::LEFT_CENTER,
                                        d,
                                        FontId::proportional(13.0),
                                        due_color(task, closed, &t),
                                    );
                                }
                                if let Some(true) = popup(ui, &dr, id.with("dp"), 380.0, |ui| {
                                    date_time_menu(
                                        ui,
                                        &t,
                                        id.with("dc"),
                                        &mut edited.due,
                                        &mut edited.due_time,
                                    )
                                }) {
                                    changed = true;
                                }
                            }
                            TableCol::Priority => {
                                let pr = ui.interact(cell, id.with("p"), Sense::click());
                                hover_bg(ui, &pr, cell.shrink(2.0), 4.0);
                                if task.priority > 0 {
                                    paint_flag(
                                        ui.painter(),
                                        cell.left_center() + vec2(10.0, 0.0),
                                        priority_color(task.priority, &t),
                                        12.0,
                                    );
                                    ui.painter().text(
                                        cell.left_center() + vec2(24.0, 0.0),
                                        Align2::LEFT_CENTER,
                                        tr(PRIORITIES[task.priority as usize].label),
                                        FontId::proportional(13.0),
                                        t.text,
                                    );
                                }
                                if let Some(true) = popup(ui, &pr, id.with("pp"), 180.0, |ui| {
                                    priority_menu(ui, &t, &mut edited.priority)
                                }) {
                                    changed = true;
                                }
                            }
                            TableCol::Tags => {
                                let mut child = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(cell.shrink2(vec2(8.0, 6.0)))
                                        .layout(Layout::left_to_right(Align::Center)),
                                );
                                child.set_clip_rect(cell);
                                // the tags that fit whole, then "+N" (hover lists all)
                                let font = FontId::proportional(11.0);
                                let gap = child.spacing().item_spacing.x;
                                let measure = child.painter().clone();
                                let width_of = |s: &str| {
                                    measure
                                        .layout_no_wrap(s.to_string(), font.clone(), t.text)
                                        .size()
                                        .x
                                        + 12.0
                                };
                                let plus_w = width_of(&format!("+{}", task.tags.len()));
                                let mut shown = 0;
                                for (k, tag) in task.tags.iter().enumerate() {
                                    let short = fit_text(&measure, tag, &font, 90.0);
                                    let rest = task.tags.len() - k - 1;
                                    let need = width_of(&short)
                                        + if rest > 0 { gap + plus_w } else { 0.0 };
                                    if need > child.available_width() {
                                        break;
                                    }
                                    chip(&mut child, &short, tag_color(tag))
                                        .on_hover_text(tag.as_str());
                                    shown += 1;
                                }
                                if shown < task.tags.len() {
                                    chip(
                                        &mut child,
                                        &format!("+{}", task.tags.len() - shown),
                                        t.text3,
                                    )
                                    .on_hover_text(task.tags.join(", "));
                                }
                            }
                            TableCol::Tracked => {
                                let secs = task.tracked_secs(now);
                                if secs > 0 {
                                    ui.painter().text(
                                        cell.left_center() + vec2(8.0, 0.0),
                                        Align2::LEFT_CENTER,
                                        fmt_duration(secs),
                                        FontId::proportional(13.0),
                                        t.text2,
                                    );
                                }
                            }
                            TableCol::Created => {
                                let created = local_date(task.created);
                                ui.painter().text(
                                    cell.left_center() + vec2(8.0, 0.0),
                                    Align2::LEFT_CENTER,
                                    fmt_date(created),
                                    FontId::proportional(13.0),
                                    t.text2,
                                );
                            }
                            TableCol::Field(fid) => {
                                let v = task.custom.get(fid).cloned().unwrap_or_default();
                                let shown = match fields
                                    .iter()
                                    .find(|f| f.id == *fid)
                                    .map(|f| f.kind.as_str())
                                {
                                    Some("checkbox") => {
                                        if v == "true" {
                                            "\u{2611}".to_string()
                                        } else {
                                            String::new()
                                        }
                                    }
                                    Some("date") => NaiveDate::parse_from_str(&v, "%Y-%m-%d")
                                        .map(fmt_date)
                                        .unwrap_or(v),
                                    _ => v,
                                };
                                ui.painter().with_clip_rect(cell.shrink(2.0)).text(
                                    cell.left_center() + vec2(8.0, 0.0),
                                    Align2::LEFT_CENTER,
                                    shown,
                                    FontId::proportional(13.0),
                                    t.text2,
                                );
                            }
                        }
                    }
                    let tid = task.id;
                    resp.context_menu(|ui| self.task_menu(ui, tid));
                    let in_box = resp
                        .interact_pointer_pos()
                        .map_or(false, |p| p.x < rr.left() + 44.0);
                    if changed {
                        self.actions.push(Action::Replace(edited));
                    } else if resp.clicked() && !in_box && !on_cell {
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
                ui.painter().text(
                    r.left_center() + vec2(8.0, 0.0),
                    Align2::LEFT_CENTER,
                    tr("+ Add task"),
                    FontId::proportional(13.0),
                    t.text3,
                );
                if resp.clicked() {
                    self.open_quick_add(None);
                }
            });
        ui.data_mut(|d| match renaming {
            Some(r) => d.insert_temp(edit_id, r),
            None => d.remove::<(u64, String)>(edit_id),
        });
    }

    // ---------------------------------------------------------------- home & inbox

    fn home(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(32, 24))
                    .show(ui, |ui| {
                        let hour = chrono::Local::now()
                            .format("%H")
                            .to_string()
                            .parse::<u32>()
                            .unwrap_or(9);
                        let greet = if hour < 12 {
                            "Good morning"
                        } else if hour < 18 {
                            "Good afternoon"
                        } else {
                            "Good evening"
                        };
                        let me = self
                            .data
                            .member(self.data.me)
                            .map(|m| m.name.clone())
                            .unwrap_or_default();
                        ui.label(
                            RichText::new(trf!("{}, {}", tr(greet), me))
                                .size(24.0)
                                .strong(),
                        );
                        ui.label(
                            RichText::new(chrono::Local::now().text("%A, %B %-d")).color(t.text2),
                        );
                        ui.add_space(18.0);
                        let td = today();
                        let mine: Vec<Task> = self
                            .data
                            .tasks
                            .iter()
                            .filter(|x| {
                                x.assignees.contains(&self.data.me)
                                    && self.task_visible(x)
                                    && !self.data.is_closed(x)
                            })
                            .cloned()
                            .collect();
                        let done_mine: Vec<Task> = self
                            .data
                            .tasks
                            .iter()
                            .filter(|x| {
                                x.assignees.contains(&self.data.me)
                                    && self.task_visible(x)
                                    && self.data.is_closed(x)
                            })
                            .cloned()
                            .collect();
                        // Section order follows the original's My Work tabs: Today, Overdue, Next, Unscheduled, Done.
                        // due today with a time that has passed counts as overdue
                        let now = now_local();
                        let mut sections: [(&'static str, Vec<&Task>); 5] = [
                            (
                                "Today",
                                mine.iter()
                                    .filter(|x| x.due == Some(td) && !x.is_overdue_at(now))
                                    .collect(),
                            ),
                            (
                                "Overdue",
                                mine.iter().filter(|x| x.is_overdue_at(now)).collect(),
                            ),
                            (
                                "Next",
                                mine.iter()
                                    .filter(|x| x.due.map(|d| d > td).unwrap_or(false))
                                    .collect(),
                            ),
                            (
                                "Unscheduled",
                                mine.iter().filter(|x| x.due.is_none()).collect(),
                            ),
                            ("Done", done_mine.iter().collect()),
                        ];
                        // today's in the order of their times, overdue and next
                        // by date (the oldest first)
                        sections[0].1.sort_by_key(|x| x.due_moment());
                        sections[1].1.sort_by_key(|x| x.due_moment());
                        sections[2].1.sort_by_key(|x| x.due_moment());
                        let avail = ui.available_width();
                        // side by side when both fit, else one above the other
                        let side = avail >= 2.0 * 320.0 + 20.0;
                        let card_w = if side { (avail - 20.0) / 2.0 } else { avail };
                        let layout = if side {
                            Layout::left_to_right(Align::Min)
                        } else {
                            Layout::top_down(Align::Min)
                        };
                        ui.with_layout(layout, |ui| {
                            // My Work card
                            egui::Frame::new()
                                .stroke(Stroke::new(1.0_f32, t.border))
                                .corner_radius(12)
                                .inner_margin(egui::Margin::same(16))
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(card_w - 34.0);
                                        ui.label(RichText::new(tr("My Work")).size(16.0).strong());
                                        ui.add_space(6.0);
                                        // nothing of mine at all: say so, not five rows of 0
                                        let (rs, ms) = self.home_due_extras();
                                        let nothing = sections.iter().all(|s| s.1.is_empty())
                                            && rs.is_empty()
                                            && ms.is_empty();
                                        if nothing {
                                            self.home_nothing_yet(ui);
                                        }
                                        for (name, list) in sections.iter().filter(|_| !nothing) {
                                            let collapsed = self.home_collapsed.contains(name);
                                            let (r, resp) = ui.allocate_exact_size(
                                                vec2(card_w - 34.0, 28.0),
                                                Sense::click(),
                                            );
                                            hover_bg(ui, &resp, r, 5.0);
                                            paint_triangle(
                                                ui.painter(),
                                                r.left_center() + vec2(6.0, 0.0),
                                                !collapsed,
                                                t.text2,
                                            );
                                            let col = if *name == "Overdue" && !list.is_empty() {
                                                t.danger
                                            } else {
                                                t.text
                                            };
                                            ui.painter().text(
                                                r.left_center() + vec2(18.0, 0.0),
                                                Align2::LEFT_CENTER,
                                                tr(name),
                                                FontId::proportional(13.5),
                                                col,
                                            );
                                            ui.painter().text(
                                                r.left_center() + vec2(110.0, 0.0),
                                                Align2::LEFT_CENTER,
                                                list.len().to_string(),
                                                FontId::proportional(12.0),
                                                t.text3,
                                            );
                                            if resp.clicked() {
                                                if collapsed {
                                                    self.home_collapsed.remove(name);
                                                } else {
                                                    self.home_collapsed.insert(name);
                                                }
                                            }
                                            if !collapsed {
                                                // a long section shows the first few and
                                                // a row to show the rest
                                                let all = self.home_all.contains(name);
                                                let cap = if all { list.len() } else { HOME_ROWS };
                                                for task in list.iter().take(cap) {
                                                    let (r, resp) = ui.allocate_exact_size(
                                                        vec2(card_w - 34.0, 30.0),
                                                        Sense::click(),
                                                    );
                                                    if resp.hovered() {
                                                        ui.painter().rect_filled(r, 6.0, t.hover);
                                                    }
                                                    let st = self.data.status_of(task);
                                                    paint_status_circle(
                                                        ui.painter(),
                                                        r.left_center() + vec2(24.0, 0.0),
                                                        6.5,
                                                        &st,
                                                    );
                                                    // right to left: the date, the list
                                                    // name, then the task name in what is left
                                                    let mut list_right = r.right() - 70.0;
                                                    if let Some(d) = fmt_due(task) {
                                                        let dr = ui.painter().text(
                                                            r.right_center() - vec2(6.0, 0.0),
                                                            Align2::RIGHT_CENTER,
                                                            d,
                                                            FontId::proportional(12.0),
                                                            due_color(task, false, &t),
                                                        );
                                                        list_right = list_right.min(dr.left() - 10.0);
                                                    }
                                                    let lr = paint_fit(
                                                        ui,
                                                        pos2(list_right, r.center().y),
                                                        Align2::RIGHT_CENTER,
                                                        &self.data.list_name(task.list_id),
                                                        FontId::proportional(11.5),
                                                        t.text3,
                                                        ((r.width() - 40.0) * 0.3).min(120.0),
                                                    );
                                                    paint_fit(
                                                        ui,
                                                        r.left_center() + vec2(40.0, 0.0),
                                                        Align2::LEFT_CENTER,
                                                        &task.name,
                                                        FontId::proportional(13.0),
                                                        t.text,
                                                        lr.left() - 10.0 - (r.left() + 40.0),
                                                    );
                                                    // the status circle ticks the task done (or opens it again)
                                                    let cr = Rect::from_center_size(
                                                        r.left_center() + vec2(24.0, 0.0),
                                                        vec2(20.0, 20.0),
                                                    );
                                                    let tick = ui.interact(
                                                        cr,
                                                        ui.id().with(("home_tick", task.id)),
                                                        Sense::click(),
                                                    );
                                                    hover_ring(ui, &tick, cr.shrink(2.0), 8.0);
                                                    let closed = self.data.is_closed(task);
                                                    name_it(
                                                        &tick,
                                                        &format!(
                                                            "{} {}",
                                                            if closed {
                                                                "Reopen"
                                                            } else {
                                                                "Complete"
                                                            },
                                                            task.name
                                                        ),
                                                    );
                                                    if tick.clicked() {
                                                        self.toggle_done(task.id);
                                                    } else if resp.clicked() {
                                                        self.actions.push(Action::Open(task.id));
                                                    }
                                                }
                                                if list.len() > HOME_ROWS {
                                                    let label = if all {
                                                        tr("Show fewer").to_string()
                                                    } else {
                                                        trf!("Show all {}", list.len())
                                                    };
                                                    let b = ui.add(ghost(egui::Button::new(
                                                        RichText::new(label).size(12.5).color(t.text2),
                                                    )));
                                                    name_it(&b, &format!("Home show all {}", name));
                                                    if b.clicked() {
                                                        if all {
                                                            self.home_all.remove(name);
                                                        } else {
                                                            self.home_all.insert(name);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        // due reminders and today's and tomorrow's meetings
                                        self.home_extras_rows(ui, card_w - 34.0);
                                    })
                                });
                            ui.add_space(12.0);
                            // Recents / lists overview
                            egui::Frame::new()
                                .stroke(Stroke::new(1.0_f32, t.border))
                                .corner_radius(12)
                                .inner_margin(egui::Margin::same(16))
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(card_w - 34.0);
                                        ui.label(RichText::new(tr("Lists")).size(16.0).strong());
                                        ui.add_space(6.0);
                                        let spaces = self.data.spaces.clone();
                                        let mut shown = 0;
                                        for s in &spaces {
                                            if s.archived
                                                || s.private_owner
                                                    .is_some_and(|o| o != self.data.me)
                                            {
                                                continue;
                                            }
                                            let lists: Vec<ListItem> = s
                                                .lists
                                                .iter()
                                                .cloned()
                                                .chain(
                                                    s.folders
                                                        .iter()
                                                        .flat_map(|f| f.lists.iter().cloned()),
                                                )
                                                .filter(|l| !self.data.list_archived(l.id))
                                                .collect();
                                            for l in lists {
                                                shown += 1;
                                                let total = self
                                                    .data
                                                    .tasks
                                                    .iter()
                                                    .filter(|x| {
                                                        x.list_id == l.id
                                                            && x.parent.is_none()
                                                            && !x.archived
                                                    })
                                                    .count();
                                                let done = self
                                                    .data
                                                    .tasks
                                                    .iter()
                                                    .filter(|x| {
                                                        x.list_id == l.id
                                                            && x.parent.is_none()
                                                            && !x.archived
                                                            && self.data.is_closed(x)
                                                    })
                                                    .count();
                                                let (r, resp) = ui.allocate_exact_size(
                                                    vec2(card_w - 34.0, 34.0),
                                                    Sense::click(),
                                                );
                                                if resp.hovered() {
                                                    ui.painter().rect_filled(r, 6.0, t.hover);
                                                }
                                                ui.painter().rect_filled(
                                                    Rect::from_center_size(
                                                        r.left_center() + vec2(14.0, 0.0),
                                                        vec2(14.0, 14.0),
                                                    ),
                                                    4.0,
                                                    rgb(s.color),
                                                );
                                                // right to left: the count and progress bar
                                                // (or for an empty List just "No tasks yet"),
                                                // then the name in what is left
                                                let name_right = if total == 0 {
                                                    let note = ui.painter().text(
                                                        r.right_center() - vec2(6.0, 0.0),
                                                        Align2::RIGHT_CENTER,
                                                        tr("No tasks yet"),
                                                        FontId::proportional(11.5),
                                                        t.text3,
                                                    );
                                                    note.left() - 10.0
                                                } else {
                                                    let count = ui.painter().text(
                                                        r.right_center() - vec2(6.0, 0.0),
                                                        Align2::RIGHT_CENTER,
                                                        format!("{}/{}", done, total),
                                                        FontId::proportional(11.5),
                                                        t.text3,
                                                    );
                                                    let bar_w =
                                                        ((r.width() - 30.0) * 0.25).min(100.0);
                                                    let bar = Rect::from_min_size(
                                                        pos2(
                                                            count.left().min(r.right() - 50.0)
                                                                - 10.0
                                                                - bar_w,
                                                            r.center().y - 3.0,
                                                        ),
                                                        vec2(bar_w, 6.0),
                                                    );
                                                    ui.painter().rect_filled(bar, 3.0, t.hover);
                                                    let f = done as f32 / total as f32;
                                                    ui.painter().rect_filled(
                                                        Rect::from_min_size(
                                                            bar.min,
                                                            vec2(bar_w * f, 6.0),
                                                        ),
                                                        3.0,
                                                        rgb([0x00, 0x88, 0x44]),
                                                    );
                                                    bar.left() - 10.0
                                                };
                                                paint_fit(
                                                    ui,
                                                    r.left_center() + vec2(30.0, 0.0),
                                                    Align2::LEFT_CENTER,
                                                    &l.name,
                                                    FontId::proportional(13.0),
                                                    t.text,
                                                    name_right - (r.left() + 30.0),
                                                );
                                                if resp.clicked() {
                                                    self.actions
                                                        .push(Action::Go(Location::List(l.id)));
                                                }
                                            }
                                        }
                                        // nothing to list yet: say so and offer the next step
                                        if shown == 0 {
                                            // Lists that are all archived are not "no Lists"
                                            let archived = spaces
                                                .iter()
                                                .filter(|s| {
                                                    s.private_owner
                                                        .is_none_or(|o| o == self.data.me)
                                                })
                                                .flat_map(|s| {
                                                    s.lists.iter().chain(
                                                        s.folders.iter().flat_map(|f| f.lists.iter()),
                                                    )
                                                })
                                                .count();
                                            let text = match archived {
                                                0 => tr("No Lists yet. Lists keep a Space's tasks together.")
                                                    .to_string(),
                                                1 => tr("The only List is archived.").to_string(),
                                                n => trf!("All {} Lists are archived.", n),
                                            };
                                            ui.label(RichText::new(text).color(t.text2));
                                            ui.add_space(8.0);
                                            ui.horizontal(|ui| {
                                                if archived > 0 {
                                                    let b = ui.add(
                                                        egui::Button::new(tr("See archived Lists"))
                                                            .stroke(Stroke::new(1.0_f32, t.border))
                                                            .corner_radius(7),
                                                    );
                                                    name_it(&b, "Home archived lists");
                                                    if b.clicked() {
                                                        self.open_settings("Spaces");
                                                    }
                                                }
                                                let has_space = !self.open_spaces().is_empty();
                                                let label = if has_space {
                                                    "+ New List"
                                                } else {
                                                    "+ New Space"
                                                };
                                                let b = ui.add(
                                                    egui::Button::new(tr(label))
                                                        .stroke(Stroke::new(1.0_f32, t.border))
                                                        .corner_radius(7),
                                                );
                                                name_it(&b, "Home new list");
                                                if b.clicked() {
                                                    self.offer_new_list(None);
                                                }
                                            });
                                        }
                                    })
                                });
                        });
                    });
            });
    }

    /// The replies Replies lists under Unread, counted for the sidebar.
    /// Asked for on every frame, so kept until the workspace changes.
    pub(super) fn unread_replies(&self) -> usize {
        self.kept_unread_replies(|| {
            let seen = self.data.extra.replies_seen;
            let read = &self.data.extra.replies_read;
            let unread = |k: String| !read.contains(&k);
            let me = self.data.me;
            let on_tasks = self
                .data
                .tasks
                .iter()
                .flat_map(|t| t.activity.iter().map(move |a| (t.id, a)))
                .filter(|(id, a)| {
                    a.comment
                        && a.author != me
                        && a.ts > seen
                        && unread(reply_key(nav::InboxSrc::Task(*id), a.ts, a.author))
                })
                .count();
            let in_chat = self
                .inbox_items()
                .iter()
                .filter(|r| r.reply && r.ts > seen && unread(reply_key(r.src, r.ts, r.author)))
                .count();
            on_tasks + in_chat
        })
    }

    fn inbox(&mut self, ui: &mut Ui, mode: u8) {
        let t = self.t();
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(24, 16))
            .show(ui, |ui| {
                let (title, sub) = match mode {
                    1 => ("Replies", "Comments from others on tasks and chat replies"),
                    2 => (
                        "Assigned Comments",
                        "Task comments assigned to someone, or that @mention them, until resolved. Chat @mentions are in Inbox.",
                    ),
                    _ => ("Inbox", "Recent activity across your workspace"),
                };
                ui.label(RichText::new(tr(title)).size(20.0).strong());
                ui.label(RichText::new(tr(sub)).color(t.text2));
                ui.add_space(8.0);
                // tabs: Unread / Read for Replies, Assigned to me / Delegated by me for comments
                let tab_id = egui::Id::new(("inbox_tab", mode));
                let mut tab: u8 = ui.data(|d| d.get_temp(tab_id)).unwrap_or(0);
                if mode != 0 {
                    ui.horizontal(|ui| {
                        let labels = if mode == 1 {
                            ["Unread", "Read"]
                        } else {
                            ["Assigned to me", "Delegated by me"]
                        };
                        for (i, l) in labels.iter().enumerate() {
                            if ui.selectable_label(tab == i as u8, tr(l)).clicked() {
                                tab = i as u8;
                            }
                        }
                        if mode == 1 && tab == 0 {
                            // off with nothing unread (known from the last frame's list)
                            let any: bool = ui
                                .data(|d| d.get_temp(egui::Id::new("replies_unread")))
                                .unwrap_or(true);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .add_enabled(any, egui::Button::new(tr("Mark all as read")))
                                    .clicked()
                                {
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
                    if icons::icon_button(ui, &t, "alarm", tr("Create Reminder"), "Create Reminder", true)
                        .clicked()
                    {
                        self.modal = Some(Modal::Reminder {
                            text: String::new(),
                            date: Some(today()),
                            time: "09:00".into(),
                        });
                    }
                }
                ui.add_space(10.0);
                // a reply marked read on its own counts as read too
                let read_list = self.data.extra.replies_read.clone();
                let read_one = |k: &str| read_list.iter().any(|x| x == k);
                let mut items: Vec<(i64, nav::InboxSrc, String, Activity)> = vec![];
                let me = self.data.me;
                let my_name = self
                    .data
                    .member(me)
                    .map(|m| format!("@{}", m.name))
                    .unwrap_or_default();
                for task in &self.data.tasks {
                    for a in &task.activity {
                        let keep = match (mode, tab) {
                            (1, 0) => {
                                a.comment
                                    && a.author != me
                                    && a.ts > seen
                                    && !read_one(&reply_key(
                                        nav::InboxSrc::Task(task.id),
                                        a.ts,
                                        a.author,
                                    ))
                            }
                            (1, _) => {
                                a.comment
                                    && a.author != me
                                    && (a.ts <= seen
                                        || read_one(&reply_key(
                                            nav::InboxSrc::Task(task.id),
                                            a.ts,
                                            a.author,
                                        )))
                            }
                            // assigned to me, or (when assigned to no one) mentioning me
                            (2, 0) => {
                                a.comment
                                    && !a.resolved
                                    && (a.assignee == Some(me)
                                        || (a.assignee.is_none()
                                            && !my_name.is_empty()
                                            && a.text.contains(&my_name)))
                            }
                            (2, _) => {
                                a.comment
                                    && !a.resolved
                                    && a.author == me
                                    && (a.assignee.is_some_and(|x| x != me)
                                        || (a.assignee.is_none() && a.text.contains('@')))
                            }
                            _ => true,
                        };
                        if keep {
                            items.push((
                                a.ts,
                                nav::InboxSrc::Task(task.id),
                                task.name.clone(),
                                a.clone(),
                            ));
                        }
                    }
                }
                // chat replies in threads I started or wrote in
                if mode == 1 {
                    for r in self.chat_replies() {
                        if (tab == 0)
                            == (r.ts > seen && !read_one(&reply_key(r.src, r.ts, r.author)))
                        {
                            items.push((
                                r.ts,
                                r.src,
                                r.title,
                                Activity {
                                    ts: r.ts,
                                    author: r.author,
                                    text: r.text,
                                    comment: true,
                                    ..Default::default()
                                },
                            ));
                        }
                    }
                }
                items.sort_by_key(|x| std::cmp::Reverse(x.0));
                if mode == 1 && tab == 0 {
                    let any = !items.is_empty();
                    ui.data_mut(|d| d.insert_temp(egui::Id::new("replies_unread"), any));
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if items.is_empty() {
                            ui.label(RichText::new(tr("You're all caught up")).color(t.text3));
                        }
                        for (ts, src, tname, a) in items.into_iter().take(200) {
                            let w = ui.available_width();
                            let (r, resp) = ui.allocate_exact_size(vec2(w, 56.0), Sense::click());
                            if resp.hovered() {
                                ui.painter().rect_filled(r, 8.0, t.hover);
                            }
                            ui.painter()
                                .hline(r.x_range(), r.bottom(), Stroke::new(0.5_f32, t.border));
                            if let Some(m) = self.data.member(a.author) {
                                paint_avatar(
                                    ui.painter(),
                                    r.left_center() + vec2(22.0, 0.0),
                                    13.0,
                                    m,
                                    t.bg,
                                );
                            }
                            ui.painter().text(
                                r.left_top() + vec2(48.0, 16.0),
                                Align2::LEFT_CENTER,
                                &tname,
                                FontId::proportional(13.5),
                                t.text,
                            );
                            let line = self.inbox_line(a.author, &a.text, a.comment, src, false);
                            ui.painter()
                                .with_clip_rect(Rect::from_min_max(
                                    r.min,
                                    pos2(r.right() - 140.0, r.bottom()),
                                ))
                                .text(
                                    r.left_top() + vec2(48.0, 38.0),
                                    Align2::LEFT_CENTER,
                                    line,
                                    FontId::proportional(12.5),
                                    t.text2,
                                );
                            ui.painter().text(
                                r.right_center() - vec2(12.0, 0.0),
                                Align2::RIGHT_CENTER,
                                fmt_ago(ts),
                                FontId::proportional(11.5),
                                t.text3,
                            );
                            // tick off an assigned comment right here
                            let mut resolved = false;
                            if let (2, nav::InboxSrc::Task(tid)) = (mode, src) {
                                let br = Rect::from_center_size(
                                    r.right_center() - vec2(118.0, 0.0),
                                    vec2(26.0, 26.0),
                                );
                                let b = put_over(ui, br, ghost(egui::Button::new("")));
                                icons::line_icon(ui.painter(), "task", br.center(), t.text2);
                                name_it(&b, &format!("Resolve comment on {}", tname));
                                if b.on_hover_text(tr("Resolve")).clicked() {
                                    if let Some(c) =
                                        self.data.tasks.iter_mut().find(|x| x.id == tid).and_then(
                                            |x| {
                                                x.activity
                                                    .iter_mut()
                                                    .find(|c| c.id == a.id && a.id != 0)
                                            },
                                        )
                                    {
                                        c.resolved = true;
                                        self.dirty = true;
                                        resolved = true;
                                    }
                                }
                            }
                            // Replies: mark one read without opening it
                            if mode == 1 && tab == 0 {
                                let br = Rect::from_min_size(
                                    pos2(r.right() - 230.0, r.center().y - 13.0),
                                    vec2(90.0, 26.0),
                                );
                                let b = put_over(
                                    ui,
                                    br,
                                    egui::Button::new(tr("Mark read")).corner_radius(6),
                                );
                                name_it(&b, &format!("Mark read {}", a.text));
                                if b.clicked() {
                                    self.data
                                        .extra
                                        .replies_read
                                        .push(reply_key(src, ts, a.author));
                                    self.dirty = true;
                                    resolved = true;
                                }
                            }
                            if resp.clicked() && !resolved {
                                if mode == 1 {
                                    let k = reply_key(src, ts, a.author);
                                    if !self.data.extra.replies_read.contains(&k) {
                                        self.data.extra.replies_read.push(k);
                                        self.dirty = true;
                                    }
                                }
                                self.open_inbox_src(src);
                            }
                        }
                    });
            });
    }

    // ---------------------------------------------------------------- task detail

    // ---------------------------------------------------------------- dialogs

    fn dialogs(&mut self, ctx: &egui::Context) {
        let t = self.t();
        // rename
        if let Some((tg, mut name)) = self.rename.take() {
            let mut open = true;
            let mut done = false;
            egui::Window::new(tr("Rename"))
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_TOP, [0.0, 120.0])
                .show(ctx, |ui| {
                    let r = ui.add(field(&mut name).desired_width(320.0));
                    r.request_focus();
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        done = true;
                    }
                    ui.horizontal(|ui| {
                        if primary_button(ui, &t, tr("Save")).clicked() {
                            done = true;
                        }
                        if ui.button(tr("Cancel")).clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
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
                Target::Space(id) => {
                    trf!("Space \"{}\"", self.data.location_name(Location::Space(id)))
                }
                Target::Folder(id) => trf!(
                    "Folder \"{}\"",
                    self.data.location_name(Location::Folder(id))
                ),
                Target::List(id) => {
                    trf!("List \"{}\"", self.data.location_name(Location::List(id)))
                }
                Target::Task(id) => trf!(
                    "task \"{}\"",
                    self.task(id).map(|x| x.name.clone()).unwrap_or_default()
                ),
            };
            egui::Window::new(tr("Delete")).collapsible(false).resizable(false).anchor(Align2::CENTER_TOP, [0.0, 120.0]).show(ctx, |ui| {
                ui.label(match tg {
                    Target::Space(_) | Target::Folder(_) => trf!("Delete {}? Its lists and their tasks go to Trash, where you can restore them.", what),
                    _ => trf!("Delete {}? It goes to Trash with everything inside it, where you can restore it.", what),
                });
                ui.horizontal(|ui| {
                    if ui.add(Tint(egui::Button::new(RichText::new(tr("Delete")).color(Color32::WHITE)).fill(t.danger))).clicked() {
                        self.do_delete(tg);
                        self.confirm_delete = None;
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        self.confirm_delete = None;
                    }
                });
            });
        }
        // statuses editor
        if let Some(sid) = self.edit_statuses {
            let mut open = true;
            if let Some(si) = self.data.spaces.iter().position(|s| s.id == sid) {
                let title = trf!("Statuses - {}", self.data.spaces[si].name);
                let mut changed = false;
                // the editor was switched to this space some other way: the
                // other space's edits are done first
                if self.status_edit.as_ref().is_some_and(|e| e.0 != sid) {
                    let other = self.status_edit.take();
                    self.settle_status_edit(other);
                }
                // the names tasks carry for each status (None: added here), so
                // renames can be passed on to the tasks when an edit is done,
                // and the names automations were last pointed at
                let (mut orig, mut rules) = self.status_edit_lists(si);
                let mut commit = false;
                let mut removed: Option<(String, String, StatusKind)> = None;
                egui::Window::new(title).open(&mut open).collapsible(false).resizable(false).anchor(Align2::CENTER_TOP, [0.0, 100.0]).show(ctx, |ui| {
                    let sts = &mut self.data.spaces[si].statuses;
                    let mut remove = None;
                    let n = sts.len();
                    for (i, st) in sts.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                            hover_bg(ui, &resp, r, 10.0);
                            paint_status_circle(ui.painter(), r.center(), 7.0, st);
                            let pid = ui.id().with(("stc", i));
                            popup(ui, &resp, pid, 170.0, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    for c in STATUS_PALETTE {
                                        let (rr, cr) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                                        ui.painter().circle_filled(rr.center(), 8.0, rgb(c));
                                        hover_ring(ui, &cr, rr.shrink(1.0), 9.0);
                                        if cr.clicked() {
                                            st.color = c;
                                            changed = true;
                                            close_popup(ui);
                                        }
                                    }
                                });
                            });
                            let name_edit = ui.add(field(&mut st.name).desired_width(160.0));
                            if name_edit.changed() {
                                changed = true;
                            }
                            if name_edit.lost_focus() {
                                commit = true;
                            }
                            egui::ComboBox::from_id_salt(("kind", i)).selected_text(tr(kind_label(st.kind))).width(90.0).show_ui(ui, |ui| {
                                for k in [StatusKind::Open, StatusKind::Custom, StatusKind::Done, StatusKind::Closed] {
                                    if ui.selectable_value(&mut st.kind, k, tr(kind_label(k))).changed() {
                                        changed = true;
                                    }
                                }
                            });
                            if n > 1 && icons::icon_button(ui, &t, "close", "", "Remove status", false).clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        let st = sts.remove(i);
                        let old = orig.remove(i).unwrap_or(st.name);
                        let rule_old = rules.remove(i).unwrap_or_else(|| old.clone());
                        removed = Some((old, rule_old, st.kind));
                        changed = true;
                    }
                    if ui.button(tr("+ Add status")).clicked() {
                        let c = STATUS_PALETTE[(sts.len() * 2 + 1) % STATUS_PALETTE.len()];
                        let pos = sts.iter().position(|s| matches!(s.kind, StatusKind::Done | StatusKind::Closed)).unwrap_or(sts.len());
                        sts.insert(pos, Status::new("new status", c, StatusKind::Custom));
                        orig.insert(pos, None);
                        rules.insert(pos, None);
                        changed = true;
                    }
                    ui.label(RichText::new(tr("Renamed statuses are renamed on their tasks too; tasks of a deleted status move to a status of the same kind.")).small().color(t.text3));
                });
                if let Some((old, rule_old, kind)) = removed {
                    // a delete finishes the other renames first
                    self.commit_status_renames(si, &mut orig, &mut rules);
                    self.move_off_status_with(si, &old, &rule_old, kind, &orig);
                }
                if commit {
                    self.commit_status_renames(si, &mut orig, &mut rules);
                } else if changed {
                    // tasks follow as names are typed, so closing the app
                    // mid-edit can't leave tasks on a name that no longer
                    // exists; automations wait for the rename to be done
                    self.follow_status_renames(si, &mut orig);
                }
                self.status_edit = Some((sid, orig, rules));
                if !open {
                    self.finish_status_edit();
                }
                if changed {
                    self.dirty = true;
                }
            }
            if !open {
                self.edit_statuses = None;
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
                    if dialog_header(ui, &t, tr("Create Task")) {
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
                    let cur = all
                        .iter()
                        .find(|x| x.0 == qa.list)
                        .map(|x| x.1.clone())
                        .unwrap_or_else(|| tr("Choose list").into());
                    let before = qa.list;
                    egui::ComboBox::from_id_salt("qa_list")
                        .selected_text(truncate(&cur, 40))
                        .show_ui(ui, |ui| {
                            for (id, name) in &all {
                                ui.selectable_value(&mut qa.list, *id, name);
                            }
                        });
                    if qa.list != before {
                        qa.status = None;
                    }
                    ui.add_space(6.0);
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut qa.name)
                            .hint_text(tr("Task Name"))
                            .font(FontId::proportional(18.0))
                            .frame(false)
                            .desired_width(f32::INFINITY),
                    );
                    name_field(&r, "Task Name");
                    focus_once(ui, &r);
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        create = true;
                    }
                    ui.add_space(4.0);
                    let dr = ui.add(
                        field_multi(&mut qa.description)
                            .hint_text(tr("Add description"))
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    name_field(&dr, "Add description");
                    ui.add_space(8.0);
                    // status, assignee, due date, priority, tags
                    let statuses = self.data.statuses_for_list(qa.list);
                    let mut st_name = qa.status.clone().unwrap_or_else(|| {
                        statuses.first().map(|s| s.name.clone()).unwrap_or_default()
                    });
                    ui.horizontal_wrapped(|ui| {
                        if let Some(st) = statuses.iter().find(|s| s.name == st_name).cloned() {
                            let r = status_pill(ui, &st, Sense::click());
                            name_it(&r, "Task status");
                            if popup(ui, &r, egui::Id::new("qa_st"), 200.0, |ui| {
                                status_menu(ui, &t, &statuses, &mut st_name)
                            })
                            .unwrap_or(false)
                            {
                                qa.status = Some(st_name.clone());
                                close_popup(ui);
                            }
                        }
                        let a_txt = if qa.assignees.is_empty() {
                            tr("Assignee").to_string()
                        } else {
                            let names: Vec<String> = qa
                                .assignees
                                .iter()
                                .filter_map(|id| self.data.member(*id).map(|m| m.name.clone()))
                                .collect();
                            truncate(&names.join(", "), 20)
                        };
                        let r = icons::icon_button(ui, &t, "person", &a_txt, "Assignee", true);
                        let data = &self.data;
                        popup(ui, &r, egui::Id::new("qa_as"), 220.0, |ui| {
                            assignee_menu(ui, &t, data, &mut qa.assignees)
                        });
                        let d_txt = qa
                            .due
                            .map(fmt_date)
                            .unwrap_or_else(|| tr("Due date").into());
                        let r = icons::icon_button(ui, &t, "calendar", &d_txt, "Due date", true);
                        popup(ui, &r, egui::Id::new("qa_due"), 380.0, |ui| {
                            date_menu(ui, &t, egui::Id::new("qa_cal"), &mut qa.due)
                        });
                        let p_txt = if qa.priority > 0 {
                            tr(PRIORITIES[qa.priority as usize].label).to_string()
                        } else {
                            tr("Priority").into()
                        };
                        let r = icons::icon_button(ui, &t, "flag", &p_txt, "Priority", true);
                        popup(ui, &r, egui::Id::new("qa_pr"), 180.0, |ui| {
                            priority_menu(ui, &t, &mut qa.priority)
                        });
                        let r = icons::icon_button(ui, &t, "tag", tr("Tags"), "Tags", true);
                        popup(ui, &r, egui::Id::new("qa_tags"), 220.0, |ui| {
                            let tag_r = ui.add(
                                field(&mut qa.tag_text)
                                    .hint_text(tr("Search or add tags..."))
                                    .desired_width(f32::INFINITY),
                            );
                            focus_once(ui, &tag_r);
                            if tag_r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let tag = qa.tag_text.trim().to_lowercase();
                                if !tag.is_empty() && !qa.tags.contains(&tag) {
                                    qa.tags.push(tag);
                                }
                                qa.tag_text.clear();
                                tag_r.request_focus();
                            }
                            let mut known: Vec<String> = self
                                .data
                                .tasks
                                .iter()
                                .flat_map(|t| t.tags.iter().cloned())
                                .collect();
                            known.sort();
                            known.dedup();
                            let q = qa.tag_text.trim().to_lowercase();
                            for tag in known
                                .iter()
                                .filter(|k| q.is_empty() || k.contains(&q))
                                .take(8)
                            {
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
                            if chip(ui, &format!("{} \u{00D7}", tag), tag_color(tag))
                                .on_hover_text(tr("Remove tag"))
                                .clicked()
                            {
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
                            if primary_button(ui, &t, tr("Create Task")).clicked() {
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
                self.actions.push(Action::Add {
                    list: qa.list,
                    name: qa.name.clone(),
                    status: qa.status.clone(),
                    priority: qa.priority,
                    assignees: qa.assignees.clone(),
                    due: qa.due,
                    parent: None,
                    open: false,
                });
            } else if open {
                self.quick_add = Some(qa);
            }
        }
        // command search
        self.search_palette(ctx);
    }

    /// One frame of the whole interface (everything except the native window buttons).
    pub(crate) fn ui_frame(&mut self, ctx: &egui::Context) {
        self.ui_frame_inner(ctx);
        crate::widgets::pointer_over_clickables(ctx);
    }

    fn ui_frame_inner(&mut self, ctx: &egui::Context) {
        self.menu_was_open = any_menu_was_open(ctx);
        self.drafts.chat.swap(self.chat_open, &mut self.chat_text);
        self.drafts
            .thread
            .swap(self.thread_open, &mut self.thread_text);
        self.drafts
            .chat_files
            .swap(self.chat_open, &mut self.chat_files);
        self.drafts
            .thread_files
            .swap(self.thread_open, &mut self.thread_files);
        self.drafts
            .chat_picked
            .swap(self.chat_open, &mut self.chat_picked);
        self.drafts
            .thread_picked
            .swap(self.thread_open, &mut self.thread_picked);
        self.files_tick(Some(ctx));
        self.personal_list_defaults();
        self.sync_view_prefs();
        if let Ok(p) = std::env::var("WORKBOARD_EVENT_LOG") {
            let evs: Vec<String> = ctx.input(|i| {
                i.raw
                    .events
                    .iter()
                    .filter(|e| {
                        !matches!(e, egui::Event::PointerMoved(_) | egui::Event::MouseMoved(_))
                    })
                    .map(|e| format!("{:?}", e))
                    .collect()
            });
            if !evs.is_empty() {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&p)
                {
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
        // shortcuts (none on the sign-in screen)
        let signing_in = self.login.is_some();
        if !signing_in
            && ctx.input(|i| {
                i.modifiers.command && (i.key_pressed(egui::Key::K) || i.key_pressed(egui::Key::J))
            })
        {
            self.palette = if self.palette.is_some() {
                None
            } else {
                Some(String::new())
            };
        }
        // Option+T types a character in a text field: only a shortcut outside one
        let typing = ctx.memory(|m| m.focused().is_some()) || ctx.wants_keyboard_input();
        if !signing_in
            && ctx.input(|i| {
                (i.modifiers.command && i.key_pressed(egui::Key::N))
                    || (!typing && i.modifiers.alt && i.key_pressed(egui::Key::T))
            })
        {
            self.open_quick_add(None);
        }
        if !signing_in && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::G)) {
            self.new_message_open = true;
        }
        if signing_in {
            // signing in: nothing of the app until then
            self.login_screen(ctx);
        } else {
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
        }
        self.toast_ui(ctx);
        // an invite link opened in a browser or mail
        // (other workboard:// links, such as Copy link, open what they point to).
        // Taken before the pending links open so it opens in this frame, and
        // drawn again at once: no input may follow to wake the app up.
        if let Some(url) = crate::links::take() {
            if url.starts_with("workboard://join") {
                self.open_join_link(&url);
            } else {
                self.link_pending = Some(url);
            }
            ctx.request_repaint();
        }
        self.open_pending_links();
        self.reminder_tick(Some(ctx));
        self.apply_actions();
        self.track_history();
        self.note_recent();
        if self.dirty {
            // lines logged with `..Default::default()` get their ids before saving
            self.data.fix_activity_ids();
            self.sync_note_change();
        }
        if self.dirty {
            self.data_changed();
            self.save_soon();
        }
        self.dirty = false;
        if self.persist {
            self.making_ws_tick();
            self.deleting_ws_tick();
            self.sync_tick(ctx);
        }
        // after sync_tick, whose merges also leave a save due, so the wake-up
        // for it is asked for in this frame
        self.save_tick(Some(ctx));
        self.huddle_tick(Some(ctx));
        self.notify_tick(Some(ctx));
        self.huddle_bar(ctx);
        // a joined workspace needs to know who you are
        // (on a server the invite code says who you are)
        if self
            .data
            .extra
            .shared
            .as_ref()
            .is_some_and(|c| c.server.is_none())
            && self.data.member(self.data.me).is_none()
            && self.modal.is_none()
        {
            self.modal = Some(Modal::WhoAmI {
                name: String::new(),
                email: String::new(),
            });
        }
        self.handle_screenshot(ctx);
        // read at the end of the pass: egui drops the focus on Escape before
        // the next pass reaches our code
        self.typing_was_on = ctx
            .memory(|m| m.focused())
            .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
    }

    fn handle_snapshot(&mut self, ctx: &egui::Context) {
        // also on request: a `snap-request` file next to the data file (used when
        // trying the app with a separate WORKBOARD_HOME folder)
        let mut pressed = ctx.input(|i| {
            i.modifiers.ctrl
                && i.modifiers.alt
                && i.modifiers.mac_cmd
                && i.key_pressed(egui::Key::S)
        });
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
        let img = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| {
                if let egui::Event::Screenshot { image, .. } = e {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
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
        let Some(path) = self.screenshot.clone() else {
            return;
        };
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
                .filter_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
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
        crate::updater::tick(ctx);
        self.ui_frame(ctx);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.catch_paste(ctx, raw_input);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.persist {
            self.finish_before_quit();
            self.save_now();
        }
    }
}

/// Saving waits until changes stop for this long, so typing a name or a
/// doc saves once instead of on every key press...
const SAVE_QUIET: std::time::Duration = std::time::Duration::from_secs(1);
/// ...but no longer than this after the first unsaved change, so steady
/// typing is still saved.
const SAVE_AT_MOST: std::time::Duration = std::time::Duration::from_secs(5);

impl App {
    /// Save the open workspace a moment from now (`save_tick`), on the
    /// background writer.
    pub(super) fn save_soon(&self) {
        if !self.persist {
            return;
        }
        let now = std::time::Instant::now();
        let first = self.save_due.get().map_or(now, |(first, _)| first);
        self.save_due.set(Some((first, now)));
    }

    /// Once a frame: hand the workspace to the background writer once the
    /// changes have settled, or wake up when they will have.
    fn save_tick(&mut self, ctx: Option<&egui::Context>) {
        let Some((first, last)) = self.save_due.get() else {
            return;
        };
        let wait = SAVE_QUIET
            .saturating_sub(last.elapsed())
            .min(SAVE_AT_MOST.saturating_sub(first.elapsed()));
        if wait.is_zero() {
            self.save_due.set(None);
            self.data.clone().save_later(Data::file_path(&self.ws_file));
        } else if let Some(ctx) = ctx {
            ctx.request_repaint_after(wait);
        }
    }

    /// Save the open workspace right away and wait for it (quitting,
    /// leaving it for another one), after anything still queued.
    pub(super) fn save_now(&self) {
        if !self.persist {
            return;
        }
        self.save_due.set(None);
        if self.ws_file == "data.json" {
            self.data.save();
        } else {
            self.data.save_file(&self.ws_file);
        }
    }

    /// Quitting: stop my running timer cleanly so tracked time is kept (only
    /// mine: a teammate's timer keeps running), and let the team have it and
    /// anything else not sent yet. The change is recorded now, so the next
    /// start can't take it as already known; a shared folder gets our copy
    /// right away (it may still be waiting out WRITE_SECS), a server gets it
    /// if the connection is up and else after the next start's welcome.
    fn finish_before_quit(&mut self) {
        // a status editor still open: its edits are done like on closing it
        self.finish_status_edit();
        self.data.stop_my_timers(now_ts());
        self.data.fix_activity_ids();
        if self.data.extra.shared.is_some() {
            self.sync_note_change();
            self.write_shared();
        }
    }
}

/// What to tell the person when a saved workspace couldn't be read.
pub(crate) fn unreadable_note(copy: &std::path::Path) -> String {
    trf!(
        "A saved workspace couldn't be read, so a copy was kept as {}.",
        copy.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    )
}

/// Home "My Work" shows this many tasks of a section until "Show all".
const HOME_ROWS: usize = 10;

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
        if i >= DUE_GROUP_CAP {
            format!("+{}", DUE_GROUP_CAP)
        } else {
            format!("{}", i)
        }
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
    let Ok(handle) = frame.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else {
        return;
    };
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
        if (c_rect.size.height - height).abs() > 0.5
            || (c_rect.origin.y - (win_rect.size.height - height)).abs() > 0.5
        {
            c_rect.size.height = height;
            c_rect.origin.y = win_rect.size.height - height;
            let _: () = msg_send![container, setFrame: c_rect];
        }
        // close at 13 px from the left edge, buttons 20 px apart
        let space = 20.0;
        for (i, b) in [close, mini, zoom].iter().enumerate() {
            let r: NSRect = msg_send![*b, frame];
            let origin = NSPoint {
                x: 13.0 + space * i as f64,
                y: r.origin.y,
            };
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
        || a.due_time != b.due_time
        || a.start_time != b.start_time
        || a.description != b.description
        || a.tags != b.tags
        || a.estimate_min != b.estimate_min
        || a.recur != b.recur
        || a.activity.len() != b.activity.len()
        || a.activity.iter().zip(b.activity.iter()).any(|(x, y)| {
            x.id != y.id
                || x.text != y.text
                || x.edited != y.edited
                || x.reactions != y.reactions
                || x.assignee != y.assignee
                || x.resolved != y.resolved
        })
        || a.checklist.len() != b.checklist.len()
        || a.custom
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .ne(b.custom.iter().filter(|(_, v)| !v.is_empty()))
        || a.checklist
            .iter()
            .zip(b.checklist.iter())
            .any(|(x, y)| x.done != y.done || x.text != y.text)
        || a.checklist
            .iter()
            .zip(b.checklist.iter())
            .any(|(x, y)| x.assignee != y.assignee)
        || a.time_entries
            .iter()
            .map(|e| (e.start, e.end))
            .ne(b.time_entries.iter().map(|e| (e.start, e.end)))
        || a.milestone != b.milestone
        || a.watchers != b.watchers
        || a.waiting_on != b.waiting_on
        || a.linked != b.linked
        || a.attachments != b.attachments
}

/// What Replies' per-item "Mark read" keeps: "taskid:ts:author" or "chat:channel:message".
fn reply_key(src: nav::InboxSrc, ts: i64, author: u64) -> String {
    match src {
        nav::InboxSrc::Task(id) => format!("{}:{}:{}", id, ts, author),
        nav::InboxSrc::Chat { ch, msg, .. } => format!("chat:{}:{}", ch, msg),
    }
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

pub(crate) fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // The original uses the system UI font (-apple-system / Segoe UI); use it when present.
    for p in [
        "/System/Library/Fonts/SFNS.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(p) {
            fonts.font_data.insert(
                "system-ui".into(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "system-ui".into());
            // a fallback for monospace text: the shortcut keys use symbols such
            // as the Option key that the built-in monospace font does not have
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("system-ui".into());
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
        // read once per run and shared: the Korean font is large (55 MB),
        // and its Bold face also comes from it
        static CJK: std::sync::OnceLock<Option<&'static [u8]>> = std::sync::OnceLock::new();
        let bytes = if p == candidates[0] {
            *CJK.get_or_init(|| {
                std::fs::read(p)
                    .ok()
                    .map(|b| &*Box::leak(b.into_boxed_slice()))
            })
        } else {
            None
        };
        let data = match bytes {
            Some(b) => egui::FontData::from_static(b),
            None => match std::fs::read(p) {
                Ok(b) => egui::FontData::from_owned(b),
                Err(_) => continue,
            },
        };
        fonts
            .font_data
            .insert("cjk".into(), std::sync::Arc::new(data));
        for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(fam).or_default().push("cjk".into());
        }
        // a bold family for big headings (the sign-in screen): Apple SD
        // Gothic Neo's Bold face, with the usual fonts after it
        if let Some(b) = bytes {
            let mut bold = egui::FontData::from_static(b);
            bold.index = 6;
            fonts
                .font_data
                .insert("cjk-bold".into(), std::sync::Arc::new(bold));
            let mut list = vec!["cjk-bold".to_string()];
            list.extend(
                fonts
                    .families
                    .get(&egui::FontFamily::Proportional)
                    .cloned()
                    .unwrap_or_default(),
            );
            fonts
                .families
                .insert(egui::FontFamily::Name(BOLD_FAMILY.into()), list);
        }
        break;
    }
    ctx.set_fonts(fonts);
}

/// The bold font family `setup_fonts` adds when the system has one.
pub(crate) const BOLD_FAMILY: &str = "bold";

/// A bold font of `size` where there is one, else the usual font.
pub(crate) fn bold_font(ctx: &egui::Context, size: f32) -> FontId {
    let fam = egui::FontFamily::Name(BOLD_FAMILY.into());
    if ctx.fonts(|f| f.families().contains(&fam)) {
        FontId::new(size, fam)
    } else {
        FontId::proportional(size)
    }
}

#[allow(dead_code)]
fn _unused(_: HashMap<u8, u8>) {}
