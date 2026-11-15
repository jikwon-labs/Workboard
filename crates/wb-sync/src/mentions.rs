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
}
