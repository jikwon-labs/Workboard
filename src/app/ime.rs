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
// - A sign or digit typed right after a syllable gets typed. For "!" right
//   after "요" the Korean IME calls insertText("요") and then
//   insertText("!") with no syllable open. winit only takes insertText while
//   a syllable is open, so it drops the "!" and holds back the key too: only
//   its release comes. A release with no press, straight after a finished
//   syllable, now types the key's character, unless the IME already put it
//   in the syllable it finished (a space comes as "요 ").
// - The same text twice: a space right after a sign comes from the IME as
//   " " and from the key as " " again. The key's copy is dropped.
// - The first key the window ever gets goes past the Korean IME and comes
//   as a "ㅇ" on its own that never joins the next letters ("ㅇㅏㄴ녕"). When
//   the window first has the keyboard the IME is handed one key and the
//   syllable taken back (`os::wake_input_method`); the IME events that
//   leaves are dropped here.

use egui::{Event, ImeEvent, Key, RawInput};

#[derive(Default)]
pub(super) struct ImeFix {
    /// input held back for the next frame
    later: Vec<Event>,
    /// Backspace went down as a key the app saw
    backspace_down: bool,
    /// what the IME just finished (no typing since): a key released
    /// without being pressed is one the IME swallowed
    committed: Option<String>,
    /// keys (where they are on the keyboard) that went down as keys the app
    /// saw: "!" can come up as "1" when Shift goes up first
    down: Vec<Key>,
    /// the Mac's input method was woken (see `os::wake_input_method`)
    pub(super) woken: bool,
    /// the IME events the waking left are still to come: drop them
    pub(super) drop_wake: bool,
}

/// The character a key types, when it is one the Korean IME passes over
/// (signs and digits). With Shift held and egui naming only the key (Num2,
/// not "@"), the Korean keyboard's signs, which sit where the US ones do.
fn typed_char(key: Key, physical: Option<Key>, modifiers: egui::Modifiers) -> Option<&'static str> {
    if modifiers.command || modifiers.ctrl || modifiers.alt {
        return None;
    }
    if modifiers.shift && physical == Some(key) {
        return Some(match key {
            Key::Num1 => "!",
            Key::Num2 => "@",
            Key::Num3 => "#",
            Key::Num4 => "$",
            Key::Num5 => "%",
            Key::Num6 => "^",
            Key::Num7 => "&",
            Key::Num8 => "*",
            Key::Num9 => "(",
            Key::Num0 => ")",
            Key::Minus => "_",
            Key::Equals => "+",
            Key::OpenBracket => "{",
            Key::CloseBracket => "}",
            Key::Backslash => "|",
            Key::Semicolon => ":",
            Key::Quote => "\"",
            Key::Comma => "<",
            Key::Period => ">",
            Key::Slash => "?",
            Key::Backtick => "~",
            Key::Space => " ",
            _ => return None,
        });
    }
    Some(match key {
        Key::Space => " ",
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        Key::Minus => "-",
        Key::Quote => "'",
        Key::Colon
        | Key::Comma
        | Key::Period
        | Key::Plus
        | Key::Equals
        | Key::Semicolon
        | Key::Backslash
        | Key::Slash
        | Key::Pipe
        | Key::Questionmark
        | Key::Exclamationmark
        | Key::OpenBracket
        | Key::CloseBracket
        | Key::OpenCurlyBracket
        | Key::CloseCurlyBracket
        | Key::Backtick => key.symbol_or_name(),
        _ => return None,
    })
}

impl ImeFix {
    /// Fix up this frame's input. True when some of it waits for the next
    /// frame (so one has to be drawn).
    pub(super) fn fix(&mut self, raw: &mut RawInput) -> bool {
        if self.drop_wake && raw.events.iter().any(|e| matches!(e, Event::Ime(_))) {
            raw.events.retain(|e| !matches!(e, Event::Ime(_)));
            self.drop_wake = false;
        }
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
        if !raw.focused {
            // releases of keys held while the window lost focus never come
            self.down.clear();
        }
        let mut i = 0;
        while i < raw.events.len() {
            match &raw.events[i] {
                Event::Ime(ImeEvent::Commit(s)) => self.committed = Some(s.clone()),
                Event::Ime(ImeEvent::Disabled) => {}
                Event::Key {
                    key,
                    physical_key,
                    pressed,
                    modifiers,
                    ..
                } => {
                    let (key, physical_key, modifiers) = (*key, *physical_key, *modifiers);
                    let at_key = physical_key.unwrap_or(key);
                    if *pressed {
                        if !self.down.contains(&at_key) {
                            self.down.push(at_key);
                        }
                        let twice = matches!(
                            (&self.committed, raw.events.get(i + 1)),
                            (Some(c), Some(Event::Text(t))) if c == t
                        );
                        if twice {
                            raw.events.remove(i + 1);
                        }
                        self.committed = None;
                    } else if let Some(at) = self.down.iter().position(|k| *k == at_key) {
                        self.down.swap_remove(at);
                    } else if let Some(c) = self.committed.take() {
                        if let Some(t) = typed_char(key, physical_key, modifiers) {
                            if !c.ends_with(t) {
                                raw.events.insert(i, Event::Text(t.into()));
                                i += 1;
                            }
                        }
                    }
                }
                Event::Text(_) | Event::Ime(ImeEvent::Preedit(_)) => self.committed = None,
                _ => {}
            }
            i += 1;
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
    fn a_sign_the_ime_swallowed_after_a_syllable_is_typed() {
        let mut fix = ImeFix::default();
        let shift = egui::Modifiers::SHIFT;
        let bang = |pressed| Event::Key {
            key: Key::Exclamationmark,
            physical_key: Some(Key::Num1),
            pressed,
            repeat: false,
            modifiers: shift,
        };
        // what the Mac sent for "!" typed while "요" was being put together
        run(&mut fix, vec![ime(ImeEvent::Preedit("요".into()))]);
        let (_, more) = run(
            &mut fix,
            vec![
                ime(ImeEvent::Preedit("요".into())),
                ime(ImeEvent::Disabled),
                ime(ImeEvent::Commit("요".into())),
                ime(ImeEvent::Disabled),
                ime(ImeEvent::Disabled),
            ],
        );
        assert!(!more);
        let (now, _) = run(&mut fix, vec![bang(false)]);
        assert_eq!(now, vec![Event::Text("!".into()), bang(false)]);
        // pressed again, the key comes whole: nothing added
        let (now, _) = run(&mut fix, vec![bang(true), Event::Text("!".into())]);
        assert_eq!(now.len(), 2);
        let (now, _) = run(&mut fix, vec![bang(false)]);
        assert_eq!(now, vec![bang(false)]);
        // "!" let go after Shift comes up as 1: still the same key
        run(&mut fix, vec![bang(true), Event::Text("!".into())]);
        let one_up = Event::Key {
            key: Key::Num1,
            physical_key: Some(Key::Num1),
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        run(&mut fix, vec![one_up]);
        run(&mut fix, vec![ime(ImeEvent::Commit("요".into()))]);
        let (now, _) = run(&mut fix, vec![bang(false)]);
        assert_eq!(now, vec![Event::Text("!".into()), bang(false)]);
        // a release with no syllable just finished adds nothing
        let (now, _) = run(&mut fix, vec![bang(false)]);
        assert_eq!(now, vec![bang(false)]);
        // Shift+2, where egui only names the key: the Korean keyboard's "@"
        run(&mut fix, vec![ime(ImeEvent::Commit("요".into()))]);
        let two = Event::Key {
            key: Key::Num2,
            physical_key: Some(Key::Num2),
            pressed: false,
            repeat: false,
            modifiers: shift,
        };
        let (now, _) = run(&mut fix, vec![two.clone()]);
        assert_eq!(now, vec![Event::Text("@".into()), two]);
    }

    #[test]
    fn a_space_the_ime_put_in_the_syllable_is_not_typed_again() {
        let mut fix = ImeFix::default();
        let space = |pressed| Event::Key {
            key: Key::Space,
            physical_key: Some(Key::Space),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        // "요" then space: the IME finishes "요 " and only the release comes
        run(&mut fix, vec![ime(ImeEvent::Preedit("요".into()))]);
        run(
            &mut fix,
            vec![
                ime(ImeEvent::Preedit("요 ".into())),
                ime(ImeEvent::Disabled),
                ime(ImeEvent::Commit("요 ".into())),
                ime(ImeEvent::Disabled),
                ime(ImeEvent::Disabled),
            ],
        );
        let (now, _) = run(&mut fix, vec![space(false)]);
        assert_eq!(now, vec![space(false)]);
        // a space right after a sign: from the IME and from the key at once
        let (now, more) = run(
            &mut fix,
            vec![
                ime(ImeEvent::Enabled),
                ime(ImeEvent::Preedit(" ".into())),
                ime(ImeEvent::Disabled),
                ime(ImeEvent::Commit(" ".into())),
                ime(ImeEvent::Disabled),
                ime(ImeEvent::Disabled),
                space(true),
                Event::Text(" ".into()),
            ],
        );
        assert!(more);
        assert_eq!(now.len(), 6);
        let (now, _) = run(&mut fix, vec![]);
        assert_eq!(now, vec![space(true)], "the key's copy of the space goes");
        // the next space is typed as usual
        run(&mut fix, vec![space(false)]);
        let (now, _) = run(&mut fix, vec![space(true), Event::Text(" ".into())]);
        assert_eq!(now.len(), 2);
    }

    #[test]
    fn the_syllable_left_by_waking_the_ime_is_dropped() {
        let mut fix = ImeFix {
            drop_wake: true,
            ..Default::default()
        };
        let (now, _) = run(&mut fix, vec![Event::Text("x".into())]);
        assert_eq!(now.len(), 1, "nothing from the IME yet");
        let (now, _) = run(
            &mut fix,
            vec![
                ime(ImeEvent::Enabled),
                ime(ImeEvent::Preedit("ㅇ".into())),
                ime(ImeEvent::Disabled),
            ],
        );
        assert!(now.is_empty());
        // typing after that is left alone
        let (now, _) = run(&mut fix, vec![ime(ImeEvent::Preedit("ㅇ".into()))]);
        assert_eq!(now.len(), 1);
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
