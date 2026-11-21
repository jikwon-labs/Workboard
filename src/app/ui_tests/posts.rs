// The Posts page: writing a post, the filters and sort, and the cards.

use super::*;

/// The Posts page open, the pointer on the window as it is for a person
/// (the harness drops the very first click after the pointer comes in).
fn posts_page() -> Ui1 {
    let mut h = Ui1::new();
    h.app.section = Section::Posts;
    h.settle();
    h.hover(pos2(900.0, 600.0));
    h
}

/// Every post in the workspace (title, text), oldest first.
fn all_posts(h: &Ui1) -> Vec<Message> {
    let mut v: Vec<Message> = h
        .app
        .data
        .channels
        .iter()
        .flat_map(|c| c.messages.iter().filter(|m| m.post_title.is_some()))
        .cloned()
        .collect();
    v.sort_by_key(|m| m.ts);
    v
}

/// Pick a channel in the post box's Channel drop-down, by clicking.
fn pick_channel(h: &mut Ui1, name: &str) {
    h.click("Post channel");
    h.click(&format!("# {name}"));
    h.settle();
}

/// Write a post in the box (channel, title, text) without sending it.
fn write_post(h: &mut Ui1, channel: &str, title: &str, text: &str) {
    pick_channel(h, channel);
    h.click("Post title");
    h.type_text(title);
    if !text.is_empty() {
        h.click("Post text");
        h.type_text(text);
    }
}

/// The page's boxes and drop-downs carry names, so a screen reader can say
/// which is which (and the tests find them).
#[test]
fn the_post_box_and_its_drop_downs_have_names() {
    let h = posts_page();
    for name in [
        "Post channel",
        "Post type",
        "Post title",
        "Post text",
        "Sort posts",
    ] {
        assert!(h.has(name), "{name} has a name");
    }
}

/// A post written on the Posts page mentions people as one written in the
/// channel does: "@channel" (and "@everyone", "@here", a team's name) as
/// well as names. It used to count only names, so "@channel" told no one.
#[test]
fn a_post_from_the_posts_page_mentions_as_the_channel_does() {
    let mut h = posts_page();
    h.app.data.extra.teams.push(Team {
        id: 900,
        name: "Design".into(),
        members: vec![3],
        color: [0, 0, 0],
    });
    write_post(
        &mut h,
        "general",
        "Office move",
        "@channel boxes go out Monday",
    );
    h.click("Post");
    let mut m = all_posts(&h).pop().expect("posted").mentions;
    m.sort();
    assert_eq!(m, vec![2, 3, 4], "@channel tells everyone in #general");

    write_post(&mut h, "website-redesign", "Mockups", "@Design please look");
    h.click("Post");
    let p = all_posts(&h).pop().unwrap();
    assert_eq!(p.mentions, vec![3], "a team's name tells the team");
}
