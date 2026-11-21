// Narrowing messages by what they carry and who wrote them: `has:file`,
// `has:link` and `from:name` in Search and in a channel's Find box, and the
// rules behind a channel panel's Files and Links.

use crate::model::{Data, Member, Message};

/// A message with pictures, videos or other files sent in it.
pub(super) fn has_files(m: &Message) -> bool {
    !m.attachments.is_empty()
}

/// A message with a web address in its text (not in code), or the preview
/// of one.
pub(super) fn has_links(m: &Message) -> bool {
    m.preview.is_some() || super::chatfmt::first_url(&m.text).is_some()
}

/// The message or one of its thread's replies passes `f` (a channel panel
/// filter shows the message its thread hangs from).
pub(super) fn in_thread(m: &Message, f: fn(&Message) -> bool) -> bool {
    f(m) || m.replies.iter().any(f)
}

/// A search text taken apart: the words to find, and the narrowing words.
#[derive(Debug, Default, PartialEq)]
pub(super) struct MsgQuery {
    /// What's left once the narrowing words are taken out, lowercased.
    pub text: String,
    pub has_file: bool,
    pub has_link: bool,
    /// The names after `from:`, lowercased; any of them will do.
    pub from: Vec<String>,
}

impl MsgQuery {
    pub(super) fn parse(q: &str) -> MsgQuery {
        let mut out = MsgQuery::default();
        let mut words = vec![];
        for w in q.split_whitespace() {
            let lw = w.to_lowercase();
            match lw.as_str() {
                "has:file" | "has:files" => out.has_file = true,
                "has:link" | "has:links" => out.has_link = true,
                _ => match lw.strip_prefix("from:") {
                    Some(name) if !name.is_empty() => {
                        out.from.push(name.trim_start_matches('@').to_string())
                    }
                    _ => words.push(lw),
                },
            }
        }
        out.text = words.join(" ");
        out
    }

    /// Whether any narrowing word was given: then only messages are found.
    pub(super) fn narrows(&self) -> bool {
        self.has_file || self.has_link || !self.from.is_empty()
    }

    /// Whether `m` has what the narrowing words ask for (the text is
    /// matched by the caller, which also looks at post titles).
    pub(super) fn fits(&self, m: &Message, data: &Data) -> bool {
        (!self.has_file || has_files(m))
            && (!self.has_link || has_links(m))
            && (self.from.is_empty() || self.from.iter().any(|f| from_matches(f, m, data)))
    }
}

/// `from:who` names the writer of `m`: "me", or the start of their name
/// (or of a word in it), or of the bot's name for a webhook's message.
fn from_matches(who: &str, m: &Message, data: &Data) -> bool {
    if let Some(bot) = &m.bot {
        return name_starts(bot, who);
    }
    if who == "me" && m.author == data.me {
        return true;
    }
    data.member(m.author).is_some_and(|p| member_starts(p, who))
}

fn member_starts(p: &Member, who: &str) -> bool {
    name_starts(&p.name, who) || name_starts(&p.display_name, who)
}

fn name_starts(name: &str, who: &str) -> bool {
    let n = name.to_lowercase();
    !n.trim().is_empty()
        && (n.replace(' ', "").starts_with(who) || n.split_whitespace().any(|w| w.starts_with(who)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrowing_words_come_out_of_the_text() {
        let q = MsgQuery::parse("Spec HAS:LINK from:alex  review has:files from:@sam");
        assert_eq!(q.text, "spec review");
        assert!(q.has_link && q.has_file);
        assert_eq!(q.from, vec!["alex", "sam"]);
        assert!(q.narrows());
        let plain = MsgQuery::parse("has: from: lunch");
        assert!(!plain.narrows());
        assert_eq!(plain.text, "has: from: lunch");
    }

    #[test]
    fn from_finds_by_name_display_name_me_and_bot() {
        let mut d = Data::blank(
            "Team",
            Member {
                id: 1,
                name: "Me Myself".into(),
                ..Default::default()
            },
        );
        d.me = 1;
        let member = |id: u64, name: &str, display: &str| {
            let mut m = Member {
                id,
                name: name.into(),
                ..Default::default()
            };
            m.display_name = display.into();
            m
        };
        d.members = vec![member(1, "Me Myself", ""), member(2, "Alex Kim", "Lex")];
        let msg = |author: u64| crate::app::slack::new_message(1, author, "hi".into(), 0);
        let fits = |q: &str, m: &Message| MsgQuery::parse(q).fits(m, &d);
        assert!(fits("from:alex", &msg(2)));
        assert!(fits("from:kim", &msg(2)), "a later word of the name");
        assert!(fits("from:alexk", &msg(2)), "the name run together");
        assert!(fits("from:lex", &msg(2)), "what they like to be called");
        assert!(!fits("from:sam", &msg(2)));
        assert!(fits("from:me", &msg(1)));
        assert!(!fits("from:me", &msg(2)));
        assert!(fits("from:sam from:alex", &msg(2)), "any of them");
        let mut bot = msg(1);
        bot.bot = Some("Deploy Bot".into());
        assert!(fits("from:deploy", &bot));
        assert!(!fits("from:me", &bot), "a webhook's message isn't mine");
    }

    #[test]
    fn has_file_and_link_look_at_what_the_message_carries() {
        let d = Data::blank(
            "Team",
            Member {
                id: 1,
                ..Default::default()
            },
        );
        let mut m = crate::app::slack::new_message(1, 1, "see `https://in.code`".into(), 0);
        assert!(!MsgQuery::parse("has:link").fits(&m, &d), "not in code");
        m.text = "see https://example.com".into();
        assert!(MsgQuery::parse("has:link").fits(&m, &d));
        assert!(!MsgQuery::parse("has:file").fits(&m, &d));
        let mut parent = crate::app::slack::new_message(2, 1, "thread".into(), 0);
        parent.replies.push(m);
        assert!(!has_links(&parent) && in_thread(&parent, has_links));
    }
}
