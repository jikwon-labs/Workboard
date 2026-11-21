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

/// With posts there but the filters hiding all of them, the page says no
/// post matches, as other filtered lists do ("No tasks match"), not that
/// there are no posts yet. Without any post it still says that.
#[test]
fn filters_that_hide_every_post_say_none_match() {
    let mut h = posts_page();
    assert!(!h.painted_text("There are no Posts yet!").is_empty());
    assert!(h.painted_text("No posts match").is_empty());
    write_post(&mut h, "general", "Office move", "Monday");
    h.click("Post");
    pick_filter(&mut h, "Post type filter", "Question");
    assert!(h.painted_text("There are no Posts yet!").is_empty());
    assert!(!h.painted_text("No posts match").is_empty());
    pick_filter(&mut h, "Post type filter", "Any type");
    assert!(h.painted_text("No posts match").is_empty());
    assert!(!h.painted_text("Office move").is_empty());
}

/// Search finds a post by its title (a post may have no text at all), as
/// finding in the channel does, and the result shows the title.
#[test]
fn search_finds_a_post_by_its_title() {
    let mut h = posts_page();
    write_post(&mut h, "general", "Quarterly kickoff", "");
    h.click("Post");
    h.app.section = Section::Home;
    h.settle();
    assert!(
        !h.has("Quarterly kickoff"),
        "not on the screen behind Search"
    );
    h.click("Search");
    h.type_text("kickoff");
    h.settle();
    assert!(h.has("Quarterly kickoff"), "the post is a result");
    h.click("Quarterly kickoff");
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.app.chat.open, Some(channel_id(&h, "general")));
}

/// A title is one line: text pasted into it with line breaks goes out with
/// spaces in their place, from the Posts page and from the channel's box.
#[test]
fn a_pasted_title_with_line_breaks_goes_out_on_one_line() {
    let mut h = posts_page();
    pick_channel(&mut h, "general");
    h.click("Post title");
    h.frame(vec![egui::Event::Paste("Office move\nMonday 9am\n".into())]);
    h.settle();
    h.click("Post");
    let p = all_posts(&h).pop().expect("posted");
    assert_eq!(p.post_title.as_deref(), Some("Office move Monday 9am"));

    h.click("general");
    h.click("Post");
    let at = h.painted_text("Title")[0].0.center();
    h.click_at(at);
    h.frame(vec![egui::Event::Paste("Lunch\nat noon".into())]);
    h.click("Message composer");
    h.type_text("Pizza");
    h.key(egui::Key::Enter);
    let p = all_posts(&h).pop().unwrap();
    assert_eq!(p.post_title.as_deref(), Some("Lunch at noon"));
}

/// A post's text shows on its card as in the channel: bold and code
/// without their marks, a mention marked.
#[test]
fn a_post_card_shows_the_texts_formatting_not_its_marks() {
    let mut h = posts_page();
    write_post(&mut h, "general", "Office move", "**Big** news for `ops`");
    h.click("Post");
    h.settle();
    assert!(
        h.painted_text("**Big** news for `ops`").is_empty(),
        "no ** or ` on the card"
    );
    assert!(!h.painted_text("Big news for ops").is_empty());
}

/// A post that came without a title (the iPhone app used to send "" when
/// only text was written) shows the default title like one sent from the
/// channel's box, on the Posts page and in the channel, not an empty line.
#[test]
fn a_post_without_a_title_shows_the_default_title() {
    let mut h = posts_page();
    let id = channel_id(&h, "general");
    for (n, title) in [(9001, ""), (9002, "  ")] {
        let mut m = crate::app::slack::new_message(n, 2, format!("From my phone {n}"), now_ts());
        m.post_title = Some(title.into());
        let c = h.app.data.channels.iter_mut().find(|c| c.id == id).unwrap();
        c.messages.push(m);
    }
    h.settle();
    assert_eq!(h.painted_text("Update").len(), 2, "on the Posts page");
    h.click("Open in channel");
    h.settle();
    assert_eq!(h.painted_text("Update").len(), 2, "in the channel");
}

/// A post by someone no longer in the workspace names them as the channel
/// does ("Someone"), so its card doesn't start with "Announcement in".
#[test]
fn a_post_by_someone_who_left_names_them_as_someone() {
    let mut h = posts_page();
    let id = channel_id(&h, "general");
    let mut m = crate::app::slack::new_message(9001, 77, "Bye all".into(), now_ts());
    m.post_title = Some("Farewell".into());
    let c = h.app.data.channels.iter_mut().find(|c| c.id == id).unwrap();
    c.messages.push(m);
    h.settle();
    assert!(!h.painted_text("Farewell").is_empty());
    assert!(
        !h.painted_text("Someone").is_empty(),
        "the author is Someone"
    );
}

/// "Open in channel" opens the post's channel the way the sidebar, Search
/// and the Inbox open a conversation: a thread left open from before
/// closes, rather than showing beside the post's channel.
#[test]
fn open_in_channel_closes_a_thread_left_open() {
    let (mut h, cid) = general_with_my_message("Who has the keys?");
    let mid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .id;
    h.app
        .apply_chat(vec![crate::app::chat::ChatAction::Thread(mid)]);
    h.settle();
    assert!(h.has("Close thread"), "the thread is open");
    h.app.section = Section::Posts;
    h.settle();
    write_post(&mut h, "general", "Office move", "Monday");
    h.click("Post");
    h.click("Open in channel");
    h.settle();
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.app.chat.open, Some(cid));
    assert!(
        !h.has("Close thread"),
        "no thread beside the post's channel"
    );
}

/// In the narrowest window the app allows, and in Korean, the filter row
/// keeps the Sort drop-down on the page: it sits at the end of the row
/// when there is room and goes under the filters when there isn't. It
/// used to run off the right edge.
#[test]
fn the_sort_drop_down_stays_on_the_page_in_a_narrow_window() {
    for ko in [false, true] {
        let mut h = Ui1::sized(900.0, 560.0);
        if ko {
            crate::i18n::set_lang(crate::i18n::Lang::Ko);
        }
        h.app.section = Section::Posts;
        h.settle();
        let sort = h.rect("Sort posts");
        let ty = h.rect("Post type filter");
        crate::i18n::set_lang(crate::i18n::Lang::En);
        assert!(
            sort.right() <= 900.0 - 8.0,
            "on the page (ko {ko}): {sort:?}"
        );
        assert!(
            !sort.intersects(ty),
            "clear of the filters: {sort:?} {ty:?}"
        );
    }
    // a wide window: at the end of the filters' row, lined up with the cards
    let mut h = posts_page();
    write_post(&mut h, "general", "Office move", "Monday");
    h.click("Post");
    let sort = h.rect("Sort posts");
    let ty = h.rect("Post type filter");
    let open = h.rect("Open in channel");
    assert!((sort.center().y - ty.center().y).abs() < 4.0, "one row");
    assert!(
        (sort.right() - open.right()).abs() < 24.0,
        "at the right like the cards: {sort:?} {open:?}"
    );
}

/// Tooltips the pointer has brought up over the window.
fn tooltips(h: &Ui1) -> usize {
    h.ctx.memory(|m| {
        m.areas()
            .visible_layer_ids()
            .into_iter()
            .filter(|l| l.order == egui::Order::Tooltip)
            .count()
    })
}

/// Resting the pointer on Post without a channel says to pick one; with a
/// channel picked no tooltip comes up (it used to be an empty bubble).
#[test]
fn the_post_button_tooltip_only_asks_for_a_channel() {
    let mut h = posts_page();
    let at = h.find("Post").unwrap();
    h.hover(at);
    for _ in 0..30 {
        h.frame(vec![]);
    }
    assert_eq!(tooltips(&h), 1);
    assert!(!h.painted_text("Select a channel first").is_empty());

    pick_channel(&mut h, "general");
    h.hover(pos2(900.0, 600.0));
    h.hover(at);
    for _ in 0..30 {
        h.frame(vec![]);
    }
    assert_eq!(tooltips(&h), 0, "no empty tooltip");
}

/// A post a minute or so old reads "1 min ago" on its card ("1분 전" in
/// Korean), not "1 mins ago".
#[test]
fn a_minute_old_post_reads_one_min_ago() {
    let mut h = posts_page();
    let id = channel_id(&h, "general");
    let mut m = crate::app::slack::new_message(9001, 2, "Coffee".into(), now_ts() - 90);
    m.post_title = Some("Break".into());
    let c = h.app.data.channels.iter_mut().find(|c| c.id == id).unwrap();
    c.messages.push(m);
    h.settle();
    let line = "Announcement in # general \u{2022} 1 min ago";
    assert!(
        !h.painted_text(line).is_empty(),
        "{:?}",
        drawn_texts(&h.shapes)
    );
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    h.settle();
    let ko = h.painted_text("# general의 공지 \u{2022} 1분 전").len();
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert_eq!(ko, 1);
}

/// Put a post in a channel as if it came from someone (newest last).
fn add_post(h: &mut Ui1, ch: u64, author: u64, title: &str, text: &str, kind: &str, ago: i64) {
    let id = h.app.data.new_id();
    let mut m = crate::app::slack::new_message(id, author, text.into(), now_ts() - ago);
    m.post_title = Some(title.into());
    m.post_type = kind.into();
    let c = h.app.data.channels.iter_mut().find(|c| c.id == ch).unwrap();
    c.messages.push(m);
}

/// The titles of the posts listed, top to bottom.
fn listed(h: &Ui1, titles: &[&str]) -> Vec<String> {
    let mut v: Vec<(f32, String)> = titles
        .iter()
        .filter_map(|t| {
            h.painted_text(t)
                .first()
                .map(|r| (r.0.top(), t.to_string()))
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v.into_iter().map(|x| x.1).collect()
}

/// The filters narrow the list together and let go one by one; a post
/// from before types were kept counts as an Announcement; Newest and
/// Oldest turn the order around.
#[test]
fn the_filters_narrow_the_posts_together_and_sort_turns_them_around() {
    let mut h = posts_page();
    let gen = channel_id(&h, "general");
    let web = channel_id(&h, "website-redesign");
    add_post(&mut h, gen, 3, "Old one", "line one\nline two", "", 400);
    add_post(&mut h, web, 2, "Web news", "Mockups", "Question", 300);
    add_post(&mut h, gen, 2, "Lunch", "Pizza", "Idea", 200);
    h.settle();
    let all = ["Old one", "Web news", "Lunch"];
    assert_eq!(
        listed(&h, &all),
        ["Lunch", "Web news", "Old one"],
        "newest first"
    );
    assert!(drawn_texts(&h.shapes)
        .iter()
        .any(|t| t.starts_with("Announcement in # general")));

    pick_filter(&mut h, "Post type filter", "Announcement");
    assert_eq!(listed(&h, &all), ["Old one"], "no type is an Announcement");
    pick_filter(&mut h, "Post type filter", "Any type");

    let me = h.app.data.me;
    let c = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == web)
        .unwrap();
    c.members.retain(|m| *m != me);
    h.click("Channels I'm Following");
    h.settle();
    assert_eq!(listed(&h, &all), ["Lunch", "Old one"], "channels I'm in");
    pick_filter(&mut h, "@  Author", "Alex Kim");
    assert_eq!(listed(&h, &all), ["Lunch"], "and by Alex");
    pick_filter(&mut h, "#  Channel", "# website-redesign");
    assert!(listed(&h, &all).is_empty());
    assert!(!h.painted_text("No posts match").is_empty());
    h.click("Channels I'm Following");
    h.settle();
    assert_eq!(listed(&h, &all), ["Web news"], "Following let go");
    pick_filter(&mut h, "@  Alex Kim", "Anyone");
    pick_filter(&mut h, "#  website-redesign", "All channels");
    assert_eq!(listed(&h, &all), ["Lunch", "Web news", "Old one"]);

    h.click("Sort posts");
    h.click("Oldest");
    h.settle();
    assert_eq!(listed(&h, &all), ["Old one", "Web news", "Lunch"]);
}

/// Cmd+Enter in the title or the text posts, as it sends a comment; Enter
/// alone still starts a new line in the text.
#[test]
fn cmd_enter_posts_from_the_title_or_the_text() {
    let mut h = posts_page();
    write_post(&mut h, "general", "Office move", "Monday");
    h.key(egui::Key::Enter);
    h.type_text("9am");
    assert!(all_posts(&h).is_empty(), "Enter is a new line");
    h.cmd_enter();
    let p = all_posts(&h).pop().expect("Cmd+Enter in the text posts");
    assert_eq!(p.text, "Monday\n9am");
    assert!(h.app.post.title.is_empty() && h.app.post.body.is_empty());

    h.click("Post title");
    h.type_text("Lunch");
    h.cmd_enter();
    assert_eq!(all_posts(&h).len(), 2, "Cmd+Enter in the title posts");
    assert_eq!(
        all_posts(&h).pop().unwrap().post_title.as_deref(),
        Some("Lunch")
    );

    // not ready (no title): nothing goes and the text stays
    h.click("Post text");
    h.type_text("No title yet");
    h.cmd_enter();
    assert_eq!(all_posts(&h).len(), 2);
    assert_eq!(h.app.post.body, "No title yet");
}
