# egui 0.31.1, patched

Vendored from crates.io egui 0.31.1 (MIT OR Apache-2.0, Emil Ernerfeldt and
the egui contributors) so text boxes edit like the Mac. Used through
`[patch.crates-io]` in the root Cargo.toml. Every change is marked with
"Workboard patch" in the code:

- `src/text_selection/text_cursor_state.rs`
  - `is_word_char`: any Unicode letter or digit, not only ASCII. Before, a
    Korean sentence was one "word", so Option+Backspace took all of it.
  - `next_word_boundary_char_index`: the macOS rule (skip spaces and
    punctuation, then the word) for Option+Backspace, Option+Delete and
    Option+Left/Right.
  - `select_word_at`: double-click selects the word (or run of other
    characters) under the pointer.
- `src/widgets/text_edit/builder.rs` (`check_for_mutating_key_press`)
  - Cmd+Backspace / Cmd+Delete delete to the start / end of the line as shown
    (the wrapped row), or only the selection when there is one. Before, they
    took the whole paragraph and more than the selection.
  - Ctrl+Backspace on the Mac deletes one letter (not a word); Ctrl+D deletes
    the next letter.

- `src/layers.rs`: `GraphicLayers::all_lists_mut` and
  `PaintList::shapes_mut`, so `src/emoji.rs` can draw color emoji over the
  black-and-white emoji glyphs in every layer just before the pass ends.

- `src/context.rs` (accesskit update in `end_pass`): the focus reported to
  the accessibility tree is the root when the focused widget made no node
  this pass (focus asked for before the widget is drawn, as a new whiteboard
  note does). Before, macOS apps reading the window (VoiceOver, automation
  tools) made accesskit_consumer panic with "Focused id ... is not in the
  node list" and the app quit.

- `src/menu.rs`: a submenu opens to the left of its parent when it does
  not fit to the right and there is more room on the left, and
  `MenuState::right_edge` hangs a menu from its right edge (`click_menu`
  lines a menu up with a button at the right edge of the window this way).
  Before, egui only opened submenus to the right.

- `src/lib.rs`: `#![allow(warnings)]`, so the old code does not fill every
  build with warnings.

When egui is upgraded, check whether upstream already does these, carry over
what it does not, and update this file.
