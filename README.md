# Workboard

A desktop work hub for small teams: spaces, lists and tasks (list, board,
calendar, Gantt and table views), chat channels and direct messages, docs,
whiteboards, forms, goals, dashboards, timesheets and more. Built with Rust
and egui; everything runs on your own computer.

## Run

    cargo run --release

## Working together

Workspace menu > Work together offers two ways:

- **A team server** (for example a Mac mini reached through Tailscale): type
  its address under "Or connect to your team's server". The first person to
  connect to an empty server becomes its owner; everyone else joins with a
  one-time invite code from Work together > People. Tasks, chat and docs show
  up for everyone within a second or two, and Huddle starts a voice and video
  call inside the app. The server also keeps backups and hands out app
  updates. Running it: `deploy/README.md`.
- **A shared folder** that a sync app (iCloud Drive, Dropbox, Google Drive,
  OneDrive, Syncthing) shares with your teammates, without a server. Tasks,
  chat and docs merge within a few seconds; calls open in the browser (Jitsi
  Meet or your own meeting link) with a Join button posted in chat.

Archived docs are also saved as Markdown in the shared folder's `Docs archive`.

## Tests

    cargo test --workspace

Logic tests, click-through tests that drive the real interface with pointer,
scroll, drag and keyboard events, and integration tests that run the real
server (voice and video tests also start `livekit-server` when it is
installed). More, including testing on another Mac: `docs/testing.md`.

## Documents

- `docs/server-design.md`: the team server's design, current state, what is
  left, and how each stage was built
- `deploy/README.md`: running the server on the Mac mini (auto start, power,
  voice, admin commands, checking after a restart)
- `docs/release/README.md`: building, notarizing and publishing app updates
- `docs/testing.md`: automated tests, hands-on checks, testing on another Mac
