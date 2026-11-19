// Formatting in chat messages: the Markdown the Docs use inside a line
// (**bold**, *italic*, ~~strike~~, `code`, [text](url) and bare web
// addresses, by `docs::inline_spans`), plus ``` code blocks, "> " quotes and
// "- " lists. The marks are hidden when a message is shown; the stored text
// keeps them, so apps that don't know formatting show it as typed. The
// iPhone app reads the same marks (ChatFormat.swift).

use super::docs::{inline_spans, Span};

/// How a run of shown text looks.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) struct Look {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    /// `code` in a line
    pub code: bool,
    /// a line of a ``` block
    pub block: bool,
    /// a "> " line
    pub quote: bool,
    pub link: bool,
}

/// A message as it is shown: the text without its marks, how each part of
/// it looks (byte ranges of `text`) and its links (character ranges, with
/// where they go).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Rich {
    pub text: String,
    pub runs: Vec<(usize, usize, Look)>,
    pub links: Vec<(usize, usize, String)>,
}

impl Rich {
    fn push(&mut self, s: &str, look: Look, url: Option<&str>) {
        if s.is_empty() {
            return;
        }
        let start = self.text.len();
        let cstart = self.text.chars().count();
        self.text.push_str(s);
        self.runs.push((start, self.text.len(), look));
        if let Some(u) = url {
            self.links
                .push((cstart, cstart + s.chars().count(), u.to_string()));
        }
    }

    /// One line's inline marks, on top of `base`.
    fn inline(&mut self, line: &str, base: Look) {
        let runs = inline_spans(line);
        // "2 * 3 * 4": marks with a space inside them are plain text
        let loose = |k: usize| {
            runs.get(k).is_some_and(|(span, t)| {
                matches!(span, Span::Bold | Span::Italic | Span::Strike)
                    && (t.starts_with(char::is_whitespace) || t.ends_with(char::is_whitespace))
            })
        };
        for (i, (span, t)) in runs.iter().enumerate() {
            let mut look = base;
            if loose(i) || (*span == Span::Marker && (loose(i + 1) || (i > 0 && loose(i - 1)))) {
                self.push(t, base, None);
                continue;
            }
            match span {
                Span::Marker => continue,
                Span::Text => {}
                Span::Bold => look.bold = true,
                Span::Italic => look.italic = true,
                Span::Strike => look.strike = true,
                Span::Code => look.code = true,
                Span::Link => {
                    // the url is in the marker run after the text: "](url)"
                    let url = runs
                        .get(i + 1)
                        .map(|m| m.1)
                        .filter(|m| m.len() > 3)
                        .map(|m| &m[2..m.len() - 1])
                        .unwrap_or("");
                    look.link = true;
                    self.push(t, look, Some(url));
                    continue;
                }
                Span::Url => {
                    look.link = true;
                    self.push(t, look, Some(t));
                    continue;
                }
            }
            self.push(t, look, None);
        }
    }
}

/// `src` as it is shown (see the top of this file).
pub(super) fn rich(src: &str) -> Rich {
    let mut out = Rich::default();
    let mut in_code = false;
    let mut first = true;
    let mut newline = |out: &mut Rich| {
        if !std::mem::take(&mut first) {
            out.push("\n", Look::default(), None);
        }
    };
    for line in src.split('\n') {
        let trimmed = line.trim_start();
        if let Some(after) = trimmed.strip_prefix("```") {
            // ```one line``` is a block of its own
            if !in_code {
                if let Some(inner) = after.strip_suffix("```").filter(|s| !s.is_empty()) {
                    newline(&mut out);
                    out.push(
                        inner,
                        Look {
                            block: true,
                            ..Default::default()
                        },
                        None,
                    );
                    continue;
                }
            }
            in_code = !in_code;
            continue;
        }
        newline(&mut out);
        if in_code {
            out.push(
                line,
                Look {
                    block: true,
                    ..Default::default()
                },
                None,
            );
        } else if let Some(rest) =
            line.strip_prefix("> ")
                .or(if line == ">" { Some("") } else { None })
        {
            let quote = Look {
                quote: true,
                ..Default::default()
            };
            out.push("\u{258E} ", quote, None);
            out.inline(rest, quote);
        } else if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            out.push("\u{2022} ", Look::default(), None);
            out.inline(rest, Look::default());
        } else {
            out.inline(line, Look::default());
        }
    }
    out
}

/// The first web address in `src` that isn't in code (for its preview).
pub(super) fn first_url(src: &str) -> Option<String> {
    rich(src)
        .links
        .into_iter()
        .map(|l| l.2)
        .find(|u| u.starts_with("https://") || u.starts_with("http://"))
}

/// `src` without its marks (notification previews, search).
pub(super) fn plain(src: &str) -> String {
    rich(src).text
}

/// Puts `mark` around the selected characters `a..b` (character indices) of
/// `text`, or two marks with the cursor between them when nothing is
/// selected. A selection over several lines gets a ``` block for "`".
/// Returns the new selection (character indices).
pub(super) fn wrap(text: &mut String, a: usize, b: usize, mark: &str) -> (usize, usize) {
    let (a, b) = (a.min(b), a.max(b));
    let byte = |c: usize, t: &str| t.char_indices().nth(c).map_or(t.len(), |x| x.0);
    let (ba, bb) = (byte(a, text), byte(b, text));
    let (open, close) = if mark == "`" && text[ba..bb].contains('\n') {
        ("```\n".to_string(), "\n```".to_string())
    } else {
        (mark.to_string(), mark.to_string())
    };
    // already wrapped: take the marks off again. Stars are counted, since
    // "**" (bold) also ends in "*": an odd run has italic, two or more bold.
    let run = |t: &str, back: bool| {
        let n = if back {
            t.chars().rev().take_while(|c| *c == '*').count()
        } else {
            t.chars().take_while(|c| *c == '*').count()
        };
        n
    };
    let present = a != b
        && match mark {
            "*" => run(&text[..ba], true) % 2 == 1 && run(&text[bb..], false) % 2 == 1,
            "**" => run(&text[..ba], true) >= 2 && run(&text[bb..], false) >= 2,
            _ => text[..ba].ends_with(&open) && text[bb..].starts_with(&close),
        };
    if present {
        text.replace_range(bb..bb + close.len(), "");
        text.replace_range(ba - open.len()..ba, "");
        let n = open.chars().count();
        return (a - n, b - n);
    }
    text.insert_str(bb, &close);
    text.insert_str(ba, &open);
    let n = open.chars().count();
    (a + n, b + n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn looks(r: &Rich) -> Vec<(&str, Look)> {
        r.runs
            .iter()
            .map(|(a, b, l)| (&r.text[*a..*b], *l))
            .collect()
    }

    #[test]
    fn marks_are_hidden_and_styles_kept() {
        let r = rich("**Ship** it *now*, ~~not~~ `cargo test` and https://example.com/x.");
        assert_eq!(
            r.text,
            "Ship it now, not cargo test and https://example.com/x."
        );
        let l = looks(&r);
        assert!(l.contains(&(
            "Ship",
            Look {
                bold: true,
                ..Default::default()
            }
        )));
        assert!(l.iter().any(|(t, l)| *t == "now" && l.italic));
        assert!(l.iter().any(|(t, l)| *t == "not" && l.strike));
        assert!(l.iter().any(|(t, l)| *t == "cargo test" && l.code));
        assert_eq!(r.links.len(), 1);
        assert_eq!(r.links[0].2, "https://example.com/x");
        let start = r.text.find("https").unwrap();
        assert_eq!(r.links[0].0, r.text[..start].chars().count());
    }

    #[test]
    fn blocks_quotes_lists_and_named_links() {
        let r = rich("look:\n```\nfn main() {}\n  x\n```\n> quoted *bit*\n- one\n* two\n[the doc](https://d.example/a) ok");
        assert_eq!(
            r.text,
            "look:\nfn main() {}\n  x\n\u{258E} quoted bit\n\u{2022} one\n\u{2022} two\nthe doc ok"
        );
        let l = looks(&r);
        assert!(l.iter().any(|(t, l)| *t == "  x" && l.block));
        assert!(l.iter().any(|(t, l)| *t == "bit" && l.quote && l.italic));
        assert_eq!(r.links[0].2, "https://d.example/a");
        assert_eq!(
            &r.text
                .chars()
                .skip(r.links[0].0)
                .take(r.links[0].1 - r.links[0].0)
                .collect::<String>(),
            "the doc"
        );
        assert_eq!(rich("```let x = 1```").text, "let x = 1");
        assert_eq!(plain("plain *text*"), "plain text");
        assert_eq!(
            plain("2 * 3 * 4"),
            "2 * 3 * 4",
            "marks with spaces inside stay"
        );
    }

    #[test]
    fn wrapping_a_selection_and_unwrapping_it() {
        let mut t = "make this bold".to_string();
        let sel = wrap(&mut t, 10, 14, "**");
        assert_eq!(t, "make this **bold**");
        assert_eq!(sel, (12, 16));
        let sel = wrap(&mut t, sel.0, sel.1, "**");
        assert_eq!(t, "make this bold", "again takes them off");
        assert_eq!(sel, (10, 14));
        let mut e = "한글 ".to_string();
        assert_eq!(wrap(&mut e, 3, 3, "*"), (4, 4));
        assert_eq!(e, "한글 **");
        let mut m = "a\nb".to_string();
        wrap(&mut m, 0, 3, "`");
        assert_eq!(m, "```\na\nb\n```");
    }
}
