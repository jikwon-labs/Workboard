// Korean (and other) input method fixes, applied to the raw input before
// egui sees it.
//
// While a syllable is still being put together (the IME "preedit"), egui
// 0.31's text box throws away Backspace and arrow keys and moves IME events
// ahead of everything else in the frame. A key that arrives in the same frame
// as the end of a syllable (a fast Backspace, a space or a letter right after
// a syllable) was lost or landed in the wrong place. And macOS only finishes
// the syllable on Cmd+Backspace: winit passes on the release of the key but
// never its press, so the box never deleted anything.
//
// - Everything after a finished syllable waits for the next frame, when the
//   box knows the syllable is done.
// - A Cmd+Backspace whose press went missing gets its press back.
// - Text the IME commits with no syllable being put together becomes plain
//   text. Typing "!" (or another sign) right after "요" makes macOS commit
//   "요" and then commit "!" as well; egui only takes a commit that ends a
//   syllable, so the "!" was dropped and only the cursor moved.

use egui::{Event, ImeEvent, Key, RawInput};

#[derive(Default)]
pub(super) struct ImeFix {
    /// input held back for the next frame
    later: Vec<Event>,
    /// Backspace went down as a key the app saw
    backspace_down: bool,
    /// a syllable is being put together (its commit is the box's to take)
    composing: bool,
}

impl ImeFix {
    /// Fix up this frame's input. True when some of it waits for the next
    /// frame (so one has to be drawn).
    pub(super) fn fix(&mut self, raw: &mut RawInput) -> bool {
        if !self.later.is_empty() {
            let mut events = std::mem::take(&mut self.later);
            events.append(&mut raw.events);
            raw.events = events;
        }
        if let Some(i) = raw
            .events
            .iter()
            .position(|e| matches!(e, Event::Ime(ImeEvent::Commit(_))))
        {
            // with the Disabled events that close it
            let mut end = i + 1;
            while matches!(raw.events.get(end), Some(Event::Ime(ImeEvent::Disabled))) {
                end += 1;
            }
            self.later = raw.events.split_off(end);
        }
        for event in &mut raw.events {
            match event {
                Event::Ime(ImeEvent::Preedit(s)) => self.composing = !s.is_empty(),
                Event::Ime(ImeEvent::Commit(s)) => {
                    if !self.composing {
                        *event = Event::Text(std::mem::take(s));
                    }
                    self.composing = false;
                }
                _ => {}
            }
        }
        let mut i = 0;
        while i < raw.events.len() {
            if let Event::Key {
                key: Key::Backspace,
                pressed,
                modifiers,
                physical_key,
                ..
            } = raw.events[i]
            {
                if pressed {
                    self.backspace_down = true;
                } else {
                    if !self.backspace_down && modifiers.command {
                        raw.events.insert(
                            i,
                            Event::Key {
                                key: Key::Backspace,
                                physical_key,
                                pressed: true,
                                repeat: false,
                                modifiers,
                            },
                        );
                        i += 1;
                    }
                    self.backspace_down = false;
                }
            }
            i += 1;
        }
        !self.later.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key, pressed: bool, modifiers: egui::Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers,
        }
    }

    fn ime(e: ImeEvent) -> Event {
        Event::Ime(e)
    }

    fn run(fix: &mut ImeFix, events: Vec<Event>) -> (Vec<Event>, bool) {
        let mut raw = RawInput {
            events,
            ..Default::default()
        };
        let more = fix.fix(&mut raw);
        (raw.events, more)
    }

    #[test]
    fn keys_after_a_finished_syllable_wait_for_the_next_frame() {
        let mut fix = ImeFix::default();
        let none = egui::Modifiers::NONE;
        // what winit sent for a Backspace right as "ㄱ" was finished
        let (now, more) = run(
            &mut fix,
            vec![
                ime(ImeEvent::Preedit("ㄱ".into())),
                ime(ImeEvent::Disabled),
                ime(ImeEvent::Commit("ㄱ".into())),
                ime(ImeEvent::Disabled),
                ime(ImeEvent::Disabled),
                key(Key::Backspace, true, none),
                key(Key::Backspace, false, none),
            ],
        );
        assert!(more);
        assert_eq!(now.len(), 5);
        assert!(matches!(now[4], Event::Ime(ImeEvent::Disabled)));
        let (next, more) = run(&mut fix, vec![]);
        assert!(!more);
        assert_eq!(
            next,
            vec![
                key(Key::Backspace, true, none),
                key(Key::Backspace, false, none)
            ]
        );
    }

    #[test]
    fn a_frame_with_two_syllables_is_spread_over_two() {
        let mut fix = ImeFix::default();
        let (now, more) = run(
            &mut fix,
            vec![
                ime(ImeEvent::Commit("안".into())),
                ime(ImeEvent::Disabled),
                ime(ImeEvent::Enabled),
                ime(ImeEvent::Preedit("녕".into())),
                ime(ImeEvent::Commit("녕".into())),
                Event::Text(" ".into()),
            ],
        );
        assert!(more);
        assert_eq!(now.len(), 2);
        // the new frame's own input comes after what waited
        let (next, more) = run(&mut fix, vec![Event::Text("x".into())]);
        assert!(more, "the second syllable splits it again");
        assert_eq!(
            next,
            vec![
                ime(ImeEvent::Enabled),
                ime(ImeEvent::Preedit("녕".into())),
                ime(ImeEvent::Commit("녕".into())),
            ]
        );
        let (last, more) = run(&mut fix, vec![]);
        assert!(!more);
        assert_eq!(last, vec![Event::Text(" ".into()), Event::Text("x".into())]);
    }

    #[test]
    fn a_sign_committed_after_a_syllable_becomes_text() {
        let mut fix = ImeFix::default();
        run(&mut fix, vec![ime(ImeEvent::Preedit("요".into()))]);
        // what winit sends for "!" typed while "요" is still being put together
        let (now, more) = run(
            &mut fix,
            vec![
                ime(ImeEvent::Disabled),
                ime(ImeEvent::Commit("요".into())),
                ime(ImeEvent::Commit("!".into())),
            ],
        );
        assert!(more);
        assert_eq!(
            now,
            vec![ime(ImeEvent::Disabled), ime(ImeEvent::Commit("요".into()))],
            "the syllable stays the box's commit"
        );
        let (next, more) = run(&mut fix, vec![]);
        assert!(!more);
        assert_eq!(
            next,
            vec![Event::Text("!".into())],
            "the sign is plain text"
        );
    }

    #[test]
    fn cmd_backspace_gets_back_the_press_macos_kept() {
        let mut fix = ImeFix::default();
        let cmd = egui::Modifiers::COMMAND;
        let (now, _) = run(&mut fix, vec![key(Key::Backspace, false, cmd)]);
        assert_eq!(
            now,
            vec![
                key(Key::Backspace, true, cmd),
                key(Key::Backspace, false, cmd)
            ]
        );
        // a whole Cmd+Backspace is left as it is
        let (now, _) = run(
            &mut fix,
            vec![
                key(Key::Backspace, true, cmd),
                key(Key::Backspace, false, cmd),
            ],
        );
        assert_eq!(now.len(), 2);
        // the press in one frame, the release in the next: still one press
        run(&mut fix, vec![key(Key::Backspace, true, cmd)]);
        let (now, _) = run(&mut fix, vec![key(Key::Backspace, false, cmd)]);
        assert_eq!(now, vec![key(Key::Backspace, false, cmd)]);
        // a plain Backspace the IME used (to take a letter off the syllable)
        // only shows its release: nothing to add, or it would delete twice
        let (now, _) = run(
            &mut fix,
            vec![key(Key::Backspace, false, egui::Modifiers::NONE)],
        );
        assert_eq!(now.len(), 1);
    }
}
