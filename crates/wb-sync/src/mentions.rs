// Who a text @mentions: one rule for the app (the Inbox, notifications,
// highlighting, the `mentions` it saves on a message) and the team server
// (iPhone pushes), so they always agree.

/// How many bytes of `rest` (the text right after an "@") are the longest
/// of `names` it starts with, by the mention rule: a Latin name running on
/// into more letters or digits ("@Alexander") is not that name; anything
/// else may follow it, so "@지원님" still mentions 지원.
pub fn name_at<'a>(rest: &str, names: impl IntoIterator<Item = &'a str>) -> Option<usize> {
    names
        .into_iter()
        .filter(|n| fits(rest, n))
        .map(|n| n.len())
        .max()
}

fn fits(rest: &str, name: &str) -> bool {
    !name.is_empty()
        && rest.starts_with(name)
        && !(name
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphanumeric())
            && rest[name.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric()))
}

/// The people (id, name) a text @mentions. At each "@" that starts a word
/// the longest matching name wins (`name_at`), so "@Alex Kim" is Alex Kim
/// and not Alex. When several people share the name, the ones in `picked`
/// (chosen from the mention list) are meant, else all of them.
pub fn mentioned_ids(text: &str, people: &[(u64, &str)], picked: &[u64]) -> Vec<u64> {
    let mut out: Vec<u64> = vec![];
    for (i, _) in text.match_indices('@') {
        if i > 0 && !text[..i].ends_with(char::is_whitespace) {
            continue;
        }
        let rest = &text[i + 1..];
        let Some(best) = name_at(rest, people.iter().map(|p| p.1)) else {
            continue;
        };
        let named: Vec<u64> = people
            .iter()
            .filter(|(_, n)| n.len() == best && fits(rest, n))
            .map(|(id, _)| *id)
            .collect();
        let chosen: Vec<u64> = named
            .iter()
            .copied()
            .filter(|id| picked.contains(id))
            .collect();
        for id in if chosen.is_empty() { named } else { chosen } {
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// The words that mention a whole group in chat: "@channel" and "@everyone"
/// (everyone in the conversation) and "@here" (who is online).
pub const EVERYONE_WORDS: &[&str] = &["channel", "everyone"];
pub const HERE_WORD: &str = "here";

/// Whether a text mentions everyone in its conversation ("@channel",
/// "@everyone") and whether it mentions who is online ("@here"), by the
/// same rule as names: at the start of a word, not running on into more
/// letters or digits.
pub fn group_mentions(text: &str) -> (bool, bool) {
    let (mut all, mut here) = (false, false);
    for (i, _) in text.match_indices('@') {
        if i > 0 && !text[..i].ends_with(char::is_whitespace) {
            continue;
        }
        let rest = &text[i + 1..];
        if EVERYONE_WORDS.iter().any(|w| fits(rest, w)) {
            all = true;
        }
        if fits(rest, HERE_WORD) {
            here = true;
        }
    }
    (all, here)
}

/// Who a chat message mentions when it is written: the people it names
/// (`mentioned_ids`), the members of each group it names ("@Design" for a
/// team called Design; a person's name of the same length wins), everyone in
/// `conversation` for "@channel" / "@everyone", and those of them `online`
/// says are online for "@here". The writer (`author`) is left out of the
/// groups. Saved on the message, so the server and the iPhone app notify
/// the same people without knowing groups or who was online.
pub fn chat_mentions(
    text: &str,
    people: &[(u64, &str)],
    groups: &[(&str, &[u64])],
    conversation: &[u64],
    online: &dyn Fn(u64) -> bool,
    author: u64,
    picked: &[u64],
) -> Vec<u64> {
    let mut out = mentioned_ids(text, people, picked);
    let add = |id: u64, out: &mut Vec<u64>| {
        if id != author && !out.contains(&id) {
            out.push(id);
        }
    };
    for (i, _) in text.match_indices('@') {
        if i > 0 && !text[..i].ends_with(char::is_whitespace) {
            continue;
        }
        let rest = &text[i + 1..];
        let Some(best) = name_at(rest, groups.iter().map(|g| g.0)) else {
            continue;
        };
        if name_at(rest, people.iter().map(|p| p.1)).is_some_and(|p| p >= best) {
            continue;
        }
        for (name, ids) in groups {
            if name.len() == best && fits(rest, name) {
                for id in ids.iter() {
                    add(*id, &mut out);
                }
            }
        }
    }
    let (all, here) = group_mentions(text);
    for id in conversation {
        if all || (here && online(*id)) {
            add(*id, &mut out);
        }
    }
    out
}

/// Whether `text` holds one of `keywords` (ignoring case; blank keywords
/// don't count): the words a person asked to be told about.
pub fn has_keyword(text: &str, keywords: &[String]) -> Option<String> {
    let low = text.to_lowercase();
    keywords
        .iter()
        .map(|k| k.trim())
        .find(|k| !k.is_empty() && low.contains(&k.to_lowercase()))
        .map(|k| k.to_string())
}

/// Who a saved message or comment mentions: the ids it keeps (`mentions`,
/// worked out when it was written) when there are any, else by its text.
pub fn mentions_of(saved: &[u64], text: &str, people: &[(u64, &str)]) -> Vec<u64> {
    if saved.is_empty() {
        mentioned_ids(text, people, &[])
    } else {
        saved.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_longest_whole_name_is_mentioned() {
        let people = [
            (1, "Alex"),
            (2, "Alex Kim"),
            (3, "Bob"),
            (4, "지원"),
            (5, "Bob"),
        ];
        assert_eq!(mentioned_ids("hi @Alex Kim!", &people, &[]), vec![2]);
        assert_eq!(mentioned_ids("hi @Alex, @Bobby", &people, &[]), vec![1]);
        assert_eq!(mentioned_ids("@지원님 확인 부탁", &people, &[]), vec![4]);
        assert!(mentioned_ids("mail@Alex", &people, &[]).is_empty());
        // two Bobs: the one picked from the list, else both
        assert_eq!(mentioned_ids("@Bob", &people, &[5]), vec![5]);
        assert_eq!(mentioned_ids("(cc) @Bob", &people, &[]), vec![3, 5]);
        assert_eq!(name_at("Alex Kim says", ["Alex", "Alex Kim"]), Some(8));
        assert_eq!(name_at("Bo", ["Bob"]), None);
        assert_eq!(mentions_of(&[9], "@Alex", &people), vec![9]);
        assert_eq!(mentions_of(&[], "@Alex", &people), vec![1]);
    }

    #[test]
    fn groups_channel_and_here_mention_their_people() {
        let people = [(1, "Alex"), (2, "Bob"), (3, "Cy"), (4, "Design Kim")];
        let design: &[u64] = &[2, 3];
        let groups = [("Design", design)];
        let conv = [1, 2, 3];
        let online = |id: u64| id == 2;
        let m = |t: &str| chat_mentions(t, &people, &groups, &conv, &online, 1, &[]);
        assert_eq!(m("@Design please look"), vec![2, 3]);
        // a longer person's name wins over the group
        assert_eq!(m("@Design Kim hi"), vec![4]);
        assert_eq!(m("@channel standup"), vec![2, 3], "everyone but the writer");
        assert_eq!(m("hey @everyone"), vec![2, 3]);
        assert_eq!(m("@here anyone?"), vec![2], "only who is online");
        assert!(m("@channels and mail@here").is_empty());
        assert_eq!(m("@Alex @here"), vec![1, 2], "a person named stays");
        assert_eq!(group_mentions("@here, @channel!"), (true, true));
        assert_eq!(group_mentions("@herewego"), (false, false));
        let kw = vec!["  ".to_string(), "Deploy".to_string()];
        assert_eq!(has_keyword("the deploy failed", &kw), Some("Deploy".into()));
        assert_eq!(has_keyword("all good", &kw), None);
    }
}
