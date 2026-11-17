# Workboard

A desktop work hub for small teams: spaces, lists and tasks (list, board,
calendar, Gantt, timeline and table views), chat channels and direct messages, docs,
whiteboards, forms, goals, dashboards, timesheets, meetings, reminders and
more. Built with Rust and egui for the Mac. Your data lives on your own
computer, and a team server of your own (or a shared folder) lets the team
work on it together. An iPhone app (`ios/`) connects to the same team server.

The interface is in English or Korean (Settings > Language; by default it
follows the system). Notifications for messages, mentions, assignments,
due tasks and meetings show as banners and a Dock badge (Settings >
Notifications).

## Run

    cargo run --release

## Working together

Workspace menu > Work together offers two ways:

- **A team server** (for example a Mac mini behind Cloudflare Tunnel at an
  https address), under
  "Or connect to your team's server":
  - **New server**: the first person types the server address, their email
    and a password. Their workspace goes up and they become the owner.
  - **Invite code**: teammates are added with Invite (their email). When the
    server has email set up it mails them a one-time code and a link;
    otherwise the code is copied from Work together > People. Joining with
    the code also sets their password.
  - **Email and password**: signs in on another computer or phone without a
    new code. "Forgot password?" mails a reset code. When the account is in
    several workspaces on that server, you choose which to open.

  One server keeps several workspaces: with a workspace on the server open,
  Create Workspace puts the new one on the same server (or, if you choose,
  on this computer only). Each has its own people, backups and files, and
  its address is the server's address plus `/w/<id>`.

  Tasks, chat and docs show up for everyone within a second or two, and
  Huddle starts a voice and video call, with screen sharing, inside the app. The server also keeps
  backups, hands out app updates and, once Apple's push service is set up,
  sends push notifications to iPhones.
  Running it: `deploy/README.md`.
- **A shared folder** that a sync app (iCloud Drive, Dropbox, Google Drive,
  OneDrive, Syncthing) shares with your teammates, without a server. Tasks,
  chat and docs merge within a few seconds; calls open in the browser (Jitsi
  Meet or your own meeting link) with a Join button posted in chat.

Archived docs are also saved as Markdown in the shared folder's `Docs archive`.

## Tests

    cargo test --workspace                       # Mac app, sync, server
    swift test --package-path ios/WorkboardKit   # iPhone app package

Logic tests, click-through tests that drive the real interface with pointer,
scroll, drag and keyboard events, and integration tests that run the real
server (voice and video tests also start `livekit-server` when it is
installed). More, including the iPhone simulator tests and testing on another
Mac: `docs/testing.md`.

## Documents

- `docs/server-design.md`: the team server's design, current state, what is
  left, and how each stage was built
- `docs/ios-design.md`: the iPhone app's design, build and tests
- `deploy/README.md`: running the server on the Mac mini (auto start, power,
  voice, email, push, admin commands, checking after a restart)
- `docs/release/README.md`: building, notarizing and publishing app updates
- `docs/testing.md`: automated tests, hands-on checks, testing on another Mac
- `docs/macbook/README.md`, `docs/macmini/README.md`: what is left to do by
  hand on the development MacBook and on the server Mac mini
  (`docs/macmini/accounts-and-mail.md`: turning on accounts and invite mail)
- `MECHANICS.md`: what was measured from the original app the interface is
  modelled on, and the rules taken from it
- `CLAUDE.md`: how changes are made in this repository (worktrees, checks,
  pushing to main)
