# Workboard

A desktop work hub for small teams: spaces, lists and tasks (list, board,
calendar, Gantt and table views), chat channels and direct messages, docs,
whiteboards, forms, goals, dashboards, timesheets and more. Built with Rust
and egui; everything runs on your own computer.

## Run

    cargo run --release

## Working together

Workspace menu > Work together: pick a folder that a sync app (iCloud Drive,
Dropbox, Google Drive, OneDrive, Syncthing) shares with your teammates.
Everyone points Workboard at the same folder; tasks, chat and docs merge
within a few seconds. Calls open in the browser (Jitsi Meet or your own
meeting link) with a Join button posted in chat. Archived docs are also saved
as Markdown in the shared folder's `Docs archive`.

## Tests

    cargo test

Logic tests plus click-through tests that drive the real interface with
pointer, scroll, drag and keyboard events.
