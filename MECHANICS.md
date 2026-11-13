# Mechanics sheet

## Measured from the person's running ClickUp (Nov 13, 2026)
The installed app was started with its debugging port, signed in, and its live
page was photographed and measured (app/shots/original-*.png). These values win
over the cached-code values further down where they differ.
- Window: 1447x949 CSS px; theme class `cu-black` (black accents).
- Top bar 40 px, white: workspace pill with a green square avatar, a planner
  icon, back/forward arrows, round search pill ("Search ⌘J"), avatar with presence dot.
- Rail: x 6, y 40, 52 px wide, solid black rgb(0,0,0), radius 8. Items Home,
  Planner, AI, Teams, More; at the bottom Labs, Install, Invite, Upgrade.
  The active item has a soft round highlight; labels white.
- Home sidebar: x 65..320 (255 px), rgb(249,249,249); header "Home" with a black
  "+ Create" button rgb(32,32,32), 14px/500 white text. Rows 28-30 px, radius 7,
  selected rgba(0,0,0,0.06). Items: Inbox, Replies, Assigned Comments, Posts, Skills,
  Artifacts, Meetings, My Tasks, More; sections AI Chats, Spaces ("All Tasks - workspace",
  spaces, lists with counts, "+ New Space"), Channels ("+ Add Channel"),
  Direct Messages ("+ New message"). Chat lives here, not on the rail.
- Page header: breadcrumb with space/list icons and a star; view tabs with coloured
  icons (List rgb(131,131,131), Board rgb(62,99,221), Calendar rgb(247,104,8),
  Gantt rgb(229,72,77), Table rgb(48,164,108)); active tab 14px/600 with a black underline.
- Toolbar: "Group: Status" pill rgb(240,241,254) with purple text, "Subtasks" pill;
  right: Columns/Sort, Filter, Closed, Assignee, me avatar, search, settings, black "Add Task".
- List: one header row (Name, Assignee, Due date, Priority, Status, Comments, +);
  group header = status pill + count; rows 36 px; empty fields show grey
  person / calendar / flag icons.
- Status pills (12px/500, uppercase): to do rgba(0,0,0,0.06) with dark text and a
  dashed circle; in progress rgb(8,128,234); complete rgb(41,151,100); white text.
- Board: columns tinted with the status colour, white cards (radius 8, 1 px border)
  with bordered assignee/date/flag boxes, "+ Add Task" in the status colour, "+ Add group".
- Task window: inset about 23 px on every side; "Task" type pill above a large title;
  fields Status / Assignees / Dates / Priority / Time estimate / Track time / Tags with
  "Empty" placeholders; "Add description"; Add subtask / Create checklist;
  right panel "Comments  Created <date>" with "Send the first comment".
- Primary button colour rgb(32,32,32); body white; border rgb(232,232,232).
- Not built (need ClickUp's online service): AI, AI Chats, Posts, Skills, Artifacts,
  Meetings, Labs, Install, Upgrade.

## Full inventory of the person's ClickUp (opened and used, Nov 13, 2026)
Every menu and page was opened in the running app (pictures: app/shots/orig-*.png).
- Top bar: workspace picker (Settings, People; Manage: Apps, Templates, Custom Fields,
  Automations, Tag Manager; Create Workspace), Planner icon, Go back / Go forward,
  "Search ⌘J" with "AI Chats", pinned tools (Create task, Record a Clip, Talk to Text),
  profile menu (Set status, Mute notifications, Settings, Notifications, Themes, ClickUp Labs,
  Keyboard shortcuts, Download ClickUp, Help; Personal Tools with pins: Create task, My Work,
  Track Time, Notepad, Record a Clip, Talk to Text, Create Reminder, Create Doc, Create
  Whiteboard, View People, Create Dashboard, AI Notetaker; Trash, Log out).
- Rail: Home, Planner, AI, Teams, More (Spaces, Chat, Docs, Dashboards, Whiteboards, Forms,
  Clips, Goals, Timesheets, Apps, Customize navigation); an app opened from More stays in the
  rail while open; bottom: Labs, Install, Invite, Upgrade.
- Sidebar header: Home, search / filter (Space, Unread, DMs) / close on hover, + Create
  (Describe anything to create; Task ⌥T, Message ⌘G; List, Channel, Space; Create with AI,
  Super Agent; Doc, Artifact, Form, Dashboard, Whiteboard, Clip; Customize your sidebar;
  Import, Templates). Items: Inbox, Replies, Assigned Comments, Posts, Skills, Artifacts,
  Meetings, My Tasks (Assigned to me, Today & Overdue, Personal List), More; AI Chats
  (Ask, Build, Create); Spaces (settings, +, All Tasks, spaces with "Create Folders, Lists,
  Docs and more", docs); Channels (Channel Settings, Add Channel); Direct Messages (DM
  Settings, New message).
- Invite: "Invite people for free", Email (comma or space separated), Invite as Member /
  Admin / Guest, Cancel, Send free invite.
- Inbox: Primary, Other, Later, Cleared; Filter, settings, Clear all; empty "Looking to
  collaborate? Collaboration is one invite away." + Invite people.
- Posts: Channel / Type (Announcement) / Title / "Share an announcement..."; Channels I'm
  Following, Channel, Author, Type, Sort Newest; "There are no Posts yet!".
- Channel: header tabs Channel, List, Board, Calendar, + View; Integrate, Ask; side panel
  Followers, Find in Channel, Replies, Pinned Messages, Assigned, Settings; empty "Chat in
  #General" with Add People, Import from Slack, Track Tasks, Add Doc, Start SyncUp;
  composer Message / Post / Ask, "Write to General, press 'space' for AI, '/' for commands".
- Docs hub: All Docs, My Docs, Shared with me, Private, Meeting Notes, Archived; Favorites;
  Popular Wikis; templates Project Overview, Meeting Notes, Wiki; table Name, Location, Tags,
  Date updated, Date viewed, Sharing. Dashboards / Whiteboards / Forms hubs: All, My (Shared,
  Private), Favorites and a 6-card template gallery. Goals: "Make your goals a reality." +
  Set a Goal. Timesheets: My timesheet, All timesheets, Approvals, Settings, week picker,
  Timesheet / Time entries, four start options. Teams: All Teams, All People, Org Chart,
  Analytics, My Teams, Create Team.
- Online-only (need the server or an AI service; shown, explained when clicked): AI, AI Chats,
  Skills, Artifacts, Super Agent, Clips, Talk to Text, AI Notetaker, SyncUp/huddles,
  Import from Slack, Integrate, calendar connection in Planner/Meetings.

## Native main program (decompiled)
`/Applications/ClickUp.app/Contents/MacOS/ClickUp` (arm64 Mach-O, 53 KB, stripped)
has 2 real functions and 2 strings (`ELECTRON_RUN_AS_NODE`, `/dev/null`).
Ghidra's decompiler isn't built for this Mac, so the built-in decompiler read them:
- `entry` @ 0x100000460: calls 0x1000004c0, then if the RunAsNode fuse is on and
  `ELECTRON_RUN_AS_NODE` is set and non-empty, starts Node
  (`ElectronInitializeICUandStartNode`), otherwise `ElectronMain`.
- `sub_1000004c0` @ 0x1000004c0: for stdin/stdout/stderr (fd 0,1,2), if `fstat`
  fails with EBADF (errno 9), reopens it on `/dev/null`.
No string matches task, status, priority, urgent or clickup. The native program
only starts the Electron engine; it holds no task rules, so nothing from it changes
the rebuild. All rules below come from the JavaScript.

The installed desktop program (v3.5.402) is an Electron shell: its `main.js`
loads the web app from the network (`appUrl: https://app.clickup.com`), so the
real interface code is not in the app bundle. The values below were read from
the web app's own scripts and stylesheets that the desktop program had cached
on this computer (decompressed, read only). There is no native machine code to
decompile, so "function addresses" are given as bundle chunk names instead.

## Priorities (chunk `init_chunk_H5N2FMGG`, file main2-6000abb682.js)
| id | label | colour |
|----|-------|--------|
| -1 | No Priority | #ffffff (shown grey here) |
| 1 | Urgent | #f50000 |
| 2 | High | #ffcc00 |
| 3 | Normal | #6fddff |
| 4 | Low | #d8d8d8 |

## Statuses (chunk `init_chunk_BL5SK4JQ`, common30-16a6bbbf3b.js)
- Status types: open, custom, done, closed (also unstarted/active/placeholder).
- Default list statuses: `to do` (open, order 0), `in progress` (custom, 1), `complete` (closed, 2).
- Simple template: `to do`, `complete`.
- Status colours (chunk `init_chunk_75VRJ7JF`, main3-b7eea52838.js):
  open grey #87909e; palette #5f55ee, #4466ff, #1090e0, #0f9d9f, #3db88b,
  #008844, #f8ae00, #e16b16, #d33d44, #ee5e99, #b660e0, #aa8d80, #656f7d.
  `to do` = E$8 #87909e, `complete` = i$2 #008844, `in progress` = s$2 #5f55ee
  (common30 imports `E$1`, `i$1`, `s` from common10.js, which exports `E$8 as E$1`,
  `i$2 as i$1`, `s$2 as s` from `init_chunk_75VRJ7JF`).
- A task with no status falls back to `{status: "to do", type: "open"}` (common24).

## Due-date grouping (function `A$1`, common11-dc5fdf6036.js; cap `B$4 = 7`)
- If the task's status type is closed or done -> group `done`.
- No due date -> `null` ("No due date").
- i = whole days from today to the due date: i == 0 -> "Today";
  i > 0 -> "+min(i, 7)"; i < 0 -> "-1" (every past day is one "Overdue" group).
- Labels (pipe `cuDueTrans`, common17-d7b3bc34eb.js): Overdue, Today, Tomorrow,
  Monday..Sunday, Future, No due date, Done.
- Rebuild: `due_group_key` and `DUE_GROUP_CAP = 7` in the app use this rule exactly.

## Week start (selector `Gn$2`, common17-7c804e7eda.js)
- `week_start_day` defaults to 0 (Sunday) when the workspace doesn't set it.
  The calendar and date picker start on Sunday.

## My Work sections (inbox chunk, common11-dc5fdf6036.js)
- Tabs in order: Today, Overdue, Next, Done, Unscheduled (plus Delegated and
  Reminders, not built). Rebuild shows Today, Overdue, Next, Unscheduled, then Done
  (folded by default).

## Closed tasks
- Built-in dashboard cards set `show_closed: false` (common34-e49fffd809.js).
  The rebuild also hides closed tasks by default; the List/Board default itself
  wasn't found, so that part is still estimated.

## Newest layout (ClickUp 4 shell, `app-loading-placeholder` rules in styles-HCC7SJPR.css)
- App background `--cu-app-v4-background` = grey50 rgb(252,252,252); header 40 px, light.
- Header: workspace/logo slot 120x28 at left; search pill height 28, fully round,
  width 20% of the window, min 11rem (176 px), max 22.5rem (360 px), centred;
  right part: quick apps, avatar 28 px.
- Body: row with gap 6 px, padding 0 6 px 6 px.
- Icon rail: 52 px wide, radius 8, padding 10 px 8 px, background gradient
  purple1000 rgb(90,67,214) -> purple1100 rgb(43,33,106) (default purple theme).
  Items: 32x32 icon box, radius 8, 22 px below each for the label (58 px wide);
  icons white (`--cu-content-on-dark`); the active item is a white box with the
  icon in purple1000 and a small shadow (`--cu-elevation-2`).
- Rail items in order (sidebar `moreMenuItems`, main3-b7eea52838.js): Spaces, Chat,
  Planner, AI, Teams, Docs, Dashboards, Whiteboards, Forms, Sheets, Clips, Goals,
  Timesheets, AI Workflows, Solutions. Built here: Home, Planner, Teams, Docs,
  Dashboards, Timesheets; the rest are listed under More as not in this version.
- Content: one white card, radius 8, 1 px border, holding the Home sidebar and the page;
  task tray at its bottom: 37 px, 8 px padding, `--cu-background-main-dark` (grey100),
  border on top, items 128 px (second 180 px) wide, radius 5.

## Chat (sidebar section labels, common47-a9e55d522c.js / main6-7ebd5444c6.js)
- Sidebar sections: `chat: "Channels"`, `dm: "Direct Messages"`; rail item `home-chat`
  titled "Chat" sits right after Spaces/Home in `moreMenuItems`.
- Message actions found in the bundles: "Reply in thread", "Add reaction", "Saved";
  forwarding uses the placeholder "Write a message (optional)".
- Rebuild: channels and direct messages, day separators, reactions (toggle your own),
  threads in a right panel, save for later, create a task from a message, unread
  count badge (messages newer than the last time you opened the conversation),
  @mentions highlighted. The quick reaction set and the badge colour (pink800) are estimated.

## Theme tokens (styles-HCC7SJPR.css)
- Main background white; strong background grey100 rgb(249,249,249) (sidebar).
- Default text grey1100 rgb(32,32,32); secondary text black 60.8% alpha.
- Default border black 9% alpha (about rgb(232,232,232) on white).
- Primary background purple800 rgb(96,73,231), hover purple900 rgb(85,62,208);
  primary content purple1000 rgb(90,67,214); subtle purple200/300.
- Overlay black 60%. Font: system UI stack (-apple-system, Segoe UI, roboto).

## Desktop shell (main.js)
- Shortcuts present in the menu: New Window Cmd+N, Command Bar, Find Cmd+F,
  Preferences Cmd+, . This rebuild maps Cmd+K to search and Cmd+N to new task.

## Estimated (looked for, not found in readable code)
- Exact layout sizes (sidebar width 264, row height 38, board column 272).
- Closed tasks hidden by default in List/Board (toggle "Closed").
- Top bar colour (purple1100 rgb(43,33,106)) and dark-theme colours.

## Click-through tests (src/app/ui_tests.rs)

The whole interface runs in a 1440x900 window and receives real pointer
moves, presses, releases, mouse-wheel scrolls, drags, right-clicks and typed
text at the on-screen position of each control (found by its accessibility
name). 23 such tests cover: sidebar navigation, Create > Task, inline
+ Add Task, status picker, opening a task and its timer/complete buttons,
Invite, view tabs, channel messages, thread replies, hover reactions, Docs,
Workspace and Profile menus with every settings dialog, search, back/forward,
Whiteboard and Form templates, Inbox Later/Clear, Timesheets tabs, board card
drag between columns, right-click Delete and Trash restore, Goals,
Dashboards, Teams and every app in More.

Bugs these found and fixed: Enter did not create a task in the Create Task
window or the inline add row (focus was re-requested before the Enter check);
the message hover toolbar pulled the following messages up over the
"1 reply" link, so it could not be clicked (all `Ui::put` overlays now use a
child Ui that does not move the layout).

## Working together without a server (src/sync.rs, src/app/together.rs)

- Workspace menu > Work together: pick a folder that a sync app shares
  (iCloud Drive, Dropbox, Google Drive, OneDrive, Syncthing; the ones found on
  the computer are offered). "Share this workspace here" writes this
  computer's copy as `<device>.workboard.json`; on another computer the same
  dialog sees it and offers "Join this workspace", which opens it as a new
  workspace (the old one stays in the workspace menu) and asks "Who are you?"
  (an invited member or someone new).
- Every 2 s the app looks at the other copies and merges them item by item:
  each item with an id keeps the newest version of its own fields, the items
  inside it (messages, replies, lists, folders) are merged one by one, task
  comments from both sides are kept, deletes leave tombstones and restoring
  revives. Read position, "saved" and sidebar expansion stay personal; the
  notepad, reminders, inbox state and trash are never written to the folder.
  New ids end in a per-computer tag so two computers never make the same id.
  Our copy is written at most every 1.5 s after a change.
- Calls: the Huddle / call buttons open "Start a call": choose the channel or
  DM, then a Jitsi Meet link (free, no account; or your own Zoom / Meet link)
  opens in the browser and a call message with Join and Copy link buttons is
  posted for everyone.
- Docs: Archive moves a doc to Archived and saves a Markdown copy in
  `<shared folder>/Docs archive` (or ~/Documents/Workboard Docs archive when
  not shared); Export saves a copy any time.
