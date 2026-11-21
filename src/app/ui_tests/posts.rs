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

fn channel_id(h: &Ui1, name: &str) -> u64 {
    h.app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Channel && c.name == name)
        .unwrap()
        .id
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

/// Pick `item` in the filter that opens from `button` (in its pop-up, not
/// a sidebar row of the same name).
fn pick_filter(h: &mut Ui1, button: &str, item: &str) {
    let b = h.rect(button);
    h.click(button);
    let below = Rect::from_min_size(pos2(b.left() - 20.0, b.bottom()), vec2(420.0, 420.0));
    let at = h.rect_in(item, below).center();
    h.click_at(at);
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

/// The channel picked for a post goes away while it is being written
/// (someone deletes it, or I leave it while it is private): the post isn't
/// sent and what was written stays. It used to clear the box and put the
/// post nowhere, or into the private channel I had left.
#[test]
fn a_post_being_written_stays_when_its_channel_goes_away() {
    let mut h = posts_page();
    write_post(&mut h, "website-redesign", "Launch", "Next week");
    let id = channel_id(&h, "website-redesign");
    h.app.data.channels.retain(|c| c.id != id);
    h.settle();
    assert!(
        !h.painted_text("Select Channel").is_empty(),
        "the box asks for a channel again"
    );
    h.click("Post");
    assert_eq!(h.app.post.title, "Launch", "the title stays");
    assert_eq!(h.app.post.body, "Next week", "the text stays");
    assert!(all_posts(&h).is_empty());

    // a private channel I leave while writing
    let mut h = posts_page();
    let id = channel_id(&h, "website-redesign");
    let c = h.app.data.channels.iter_mut().find(|c| c.id == id).unwrap();
    c.private = true;
    h.settle();
    write_post(&mut h, "website-redesign", "Launch", "Next week");
    let me = h.app.data.me;
    let c = h.app.data.channels.iter_mut().find(|c| c.id == id).unwrap();
    c.members.retain(|m| *m != me);
    h.settle();
    h.click("Post");
    assert!(
        all_posts(&h).is_empty(),
        "nothing went into the channel I left"
    );
    assert_eq!(h.app.post.title, "Launch");

    // picking a channel again sends it
    pick_channel(&mut h, "general");
    h.click("Post");
    assert_eq!(all_posts(&h).len(), 1);
    assert!(h.app.post.title.is_empty());
}

/// The channel's Post box and the Posts page each keep their own title: a
/// title being written on the page doesn't turn up in the channel's box
/// (and go out with its post), and sending there doesn't clear it here.
#[test]
fn the_posts_page_and_the_channel_post_box_keep_their_own_titles() {
    let mut h = posts_page();
    h.click("Post title");
    h.type_text("Draft on the page");
    h.click("general");
    h.click("Post");
    assert!(
        h.painted_text("Draft on the page").is_empty(),
        "the channel's box starts without the page's title"
    );
    h.click("Message composer");
    h.type_text("Hello from the channel");
    h.key(egui::Key::Enter);
    let p = all_posts(&h).pop().unwrap();
    assert_eq!(p.post_title.as_deref(), Some("Update"), "its own title");
    assert_eq!(p.text, "Hello from the channel");
    h.app.section = Section::Posts;
    h.settle();
    assert_eq!(
        h.app.post.title, "Draft on the page",
        "the page's title stays"
    );
    assert!(!h.painted_text("Draft on the page").is_empty());
}

/// A channel or person picked in the filters goes away (deleted, left, or
/// taken out of the workspace): the filter lets go and the posts show
/// again. The buttons already looked unfiltered ("#  Channel", "@  Author")
/// while every post stayed hidden.
#[test]
fn a_filter_on_a_channel_or_person_that_is_gone_lets_go() {
    let mut h = posts_page();
    write_post(&mut h, "general", "Office move", "Monday");
    h.click("Post");
    pick_filter(&mut h, "#  Channel", "# website-redesign");
    assert!(h.painted_text("Office move").is_empty(), "filtered out");
    let id = channel_id(&h, "website-redesign");
    h.app.data.channels.retain(|c| c.id != id);
    h.settle();
    assert!(h.has("#  Channel"));
    assert!(
        !h.painted_text("Office move").is_empty(),
        "the post shows again"
    );

    pick_filter(&mut h, "@  Author", "Alex Kim");
    assert!(h.painted_text("Office move").is_empty(), "filtered out");
    h.app.data.members.retain(|m| m.id != 2);
    h.settle();
    assert!(h.has("@  Author"));
    assert!(
        !h.painted_text("Office move").is_empty(),
        "the post shows again"
    );
}

/// Another workspace opened: its channels and people are others, so the
/// channel picked for the post and the channel and person filters start
/// over (another workspace's ids may name something else there). What was
/// written stays, to be sent once a channel of this workspace is picked.
#[test]
fn another_workspace_starts_the_posts_page_channel_and_filters_over() {
    let _home = crate::app::tests::HOME_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let home = temp_home("home-posts-ws");
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let mut h = posts_page();
    write_post(&mut h, "general", "Office move", "Monday");
    pick_filter(&mut h, "#  Channel", "# general");
    pick_filter(&mut h, "@  Author", "Alex Kim");
    assert!(h.app.post.f_channel.is_some() && h.app.post.f_author.is_some());
    h.click("Workspace menu");
    h.click("Create Workspace");
    h.type_text("Side Biz");
    h.click("Next");
    h.click("Create");
    h.settle();
    assert_eq!(h.app.data.workspace, "Side Biz");
    assert_eq!(h.app.post.channel, 0, "no channel picked here yet");
    assert_eq!(h.app.post.f_channel, None);
    assert_eq!(h.app.post.f_author, None);
    assert_eq!(h.app.post.title, "Office move", "what was written stays");
    assert_eq!(h.app.post.body, "Monday");
}
