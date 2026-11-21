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
    crate::app::tests::point_home(&home);
    let mut h = posts_page();
    write_post(&mut h, "general", "Office move", "Monday");
    pick_filter(&mut h, "#  Channel", "# general");
    pick_filter(&mut h, "@  Author", "Alex Kim");
    assert!(h.app.post.f_channel.is_some() && h.app.post.f_author.is_some());
    h.app.post.picked = vec![2];
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
    assert!(
        h.app.post.picked.is_empty(),
        "nobody picked from its @ list"
    );
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
    assert!(!h.painted_text(line).is_empty(), "{:?}", h.texts());
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
    assert!(h
        .texts()
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

/// "New Post" puts the cursor in the title, even from far down the list,
/// and keeps what was being written (it used to clear it without asking
/// and leave the keyboard nowhere).
#[test]
fn new_post_puts_the_cursor_in_the_title_and_keeps_what_was_written() {
    let mut h = posts_page();
    h.click("New Post");
    h.type_text("Hello");
    assert_eq!(h.app.post.title, "Hello", "typing goes into the title");

    let gen = channel_id(&h, "general");
    for i in 0..20 {
        add_post(
            &mut h,
            gen,
            2,
            &format!("Post {i}"),
            "text",
            "Idea",
            i * 60 + 60,
        );
    }
    h.click("Post text");
    h.type_text("Draft");
    // down the list, the box out of sight
    h.frame(vec![egui::Event::PointerMoved(pos2(900.0, 600.0))]);
    for _ in 0..10 {
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -400.0),
            modifiers: Default::default(),
        }]);
    }
    h.settle();
    assert!(
        h.find("Post title").is_none_or(|p| p.y < 60.0),
        "scrolled away"
    );
    h.click("New Post");
    h.settle();
    assert_eq!(h.app.post.title, "Hello", "kept");
    assert_eq!(h.app.post.body, "Draft", "kept");
    let at = h.find("Post title").unwrap();
    assert!(
        at.y > 60.0 && at.y < 900.0,
        "the title is back in sight: {at:?}"
    );
    h.type_text("!");
    assert_eq!(h.app.post.title, "Hello!");
}

/// The buttons under the post's text work as in a channel's box: "@" lists
/// the people (and @channel, @here, teams) to mention, the smile lists
/// emoji, each put in where the cursor is; typing "@" opens the people
/// list too. There is one Create task button (a "+" did the same thing),
/// and a person picked from the list is mentioned.
#[test]
fn the_post_box_buttons_work_as_in_a_channels_box() {
    let mut h = posts_page();
    let composer = Rect::from_min_max(pos2(340.0, 100.0), pos2(1440.0, 520.0));
    let in_box = |h: &Ui1, name: &str| {
        h.tree
            .iter()
            .filter(|(n, r)| n == name && composer.contains(r.center()))
            .count()
    };
    assert_eq!(in_box(&h, "+"), 0, "no second Create task button");
    assert_eq!(in_box(&h, "Create task"), 1);

    write_post(&mut h, "general", "Office move", "Hello world");
    // the cursor back before " world"
    for _ in 0..6 {
        h.key(egui::Key::ArrowLeft);
    }
    let at = h.rect("Mention someone");
    h.click("Mention someone");
    let list = Rect::from_min_size(pos2(at.left() - 20.0, at.bottom()), vec2(420.0, 360.0));
    assert!(
        h.tree
            .iter()
            .any(|(n, r)| n == "@channel" && list.contains(r.center())),
        "the groups are listed too"
    );
    let alex = h.rect_in("Alex Kim", list).center();
    h.click_at(alex);
    h.settle();
    assert_eq!(h.app.post.body, "Hello @Alex Kim  world", "at the cursor");
    h.type_text("x");
    assert_eq!(
        h.app.post.body, "Hello @Alex Kim x world",
        "the box keeps the keyboard"
    );

    // typing "@" at the start of a word opens the list
    h.key(egui::Key::End);
    h.type_text(" @");
    h.settle();
    let sam = h.rect_in("Sam Lee", list).center();
    h.click_at(sam);
    h.settle();
    assert_eq!(h.app.post.body, "Hello @Alex Kim x world @Sam Lee ");

    h.click("Emoji");
    h.click("\u{2615}");
    h.settle();
    assert_eq!(h.app.post.body, "Hello @Alex Kim x world @Sam Lee \u{2615}");

    h.click("Post");
    let mut m = all_posts(&h).pop().unwrap().mentions;
    m.sort();
    assert_eq!(m, vec![2, 3], "the people picked are mentioned");
}

/// A channel's workflows ("When a message is posted ...") start from a
/// post too, from the Posts page and from the channel's Post box: the
/// keyword may be in the title or the text, and {text} is the post's text
/// (its title when it has none).
#[test]
fn a_post_starts_the_channels_workflows() {
    let mut h = posts_page();
    let gen = channel_id(&h, "general");
    let web = channel_id(&h, "website-redesign");
    h.app.data.extra.automations.push(Automation {
        id: 77_001,
        enabled: true,
        trigger: "message".into(),
        channel: Some(gen),
        keyword: "outage".into(),
        action: "post".into(),
        target: Some(web),
        value: "{user} posted: {text}".into(),
        ..Default::default()
    });
    let me = h.app.who(h.app.data.me);
    let last_in_web = |h: &Ui1| {
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == web)
            .unwrap()
            .messages
            .last()
            .map(|m| (m.text.clone(), m.bot.clone()))
    };
    write_post(&mut h, "general", "Outage tonight", "DB maintenance at 10");
    h.click("Post");
    assert_eq!(
        last_in_web(&h),
        Some((
            format!("{me} posted: DB maintenance at 10"),
            Some("Workflow".into())
        )),
        "the keyword in the title starts it"
    );
    write_post(&mut h, "general", "Outage over", "");
    h.click("Post");
    assert_eq!(
        last_in_web(&h).unwrap().0,
        format!("{me} posted: Outage over"),
        "no text: the title"
    );
    write_post(&mut h, "general", "Lunch", "Pizza");
    h.click("Post");
    assert_eq!(
        last_in_web(&h).unwrap().0,
        format!("{me} posted: Outage over"),
        "no keyword"
    );

    // from the channel's Post box
    h.click("general");
    h.click("Post");
    h.click("Message composer");
    h.type_text("Another outage");
    h.key(egui::Key::Enter);
    assert_eq!(
        last_in_web(&h).unwrap().0,
        format!("{me} posted: Another outage")
    );
}

/// A post sent with pictures (from a channel's Post box) shows them on its
/// card, as in the channel, and a click opens one large. The card used to
/// show only the title, so a post of pictures alone looked empty.
#[test]
fn a_post_card_shows_its_pictures_and_opens_them() {
    let mut h = posts_page();
    let gen = channel_id(&h, "general");
    add_post(&mut h, gen, 2, "Update", "", "Announcement", 60);
    let c = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == gen)
        .unwrap();
    c.messages.last_mut().unwrap().attachments.push(Attachment {
        file: "0".repeat(64),
        name: "beach.jpg".into(),
        kind: MediaKind::Image,
        bytes: 1000,
        w: 800,
        h: 600,
        secs: 0.0,
        thumb: None,
    });
    h.settle();
    assert!(h.has("Picture beach.jpg"), "the picture is on the card");
    h.click("Picture beach.jpg");
    assert!(h.app.viewer.is_some(), "and opens large");
    h.key(egui::Key::Escape);
    assert!(h.app.viewer.is_none());
    assert_eq!(h.app.section, Section::Posts, "still on the Posts page");
}

/// The title the app gives a post sent without one ("Update") isn't
/// something anyone wrote: a workflow's keyword isn't found in it, and a
/// post of files alone has no {text} from it (nor a task named "Update").
#[test]
fn the_default_post_title_starts_no_workflow() {
    let mut h = posts_page();
    let gen = channel_id(&h, "general");
    let web = channel_id(&h, "website-redesign");
    let list = h.app.data.first_list().unwrap();
    for (id, keyword, action, value) in [
        (
            77_001,
            "update",
            "post",
            "{user} posted: {text}".to_string(),
        ),
        (77_002, "", "task", list.to_string()),
    ] {
        h.app.data.extra.automations.push(Automation {
            id,
            enabled: true,
            trigger: "message".into(),
            channel: Some(gen),
            keyword: keyword.into(),
            action: action.into(),
            target: Some(web),
            value,
            ..Default::default()
        });
    }
    let in_web = |h: &Ui1| {
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == web)
            .unwrap()
            .messages
            .len()
    };
    let (before, tasks) = (in_web(&h), h.app.data.tasks.len());
    // from the channel's Post box, without a title: saved as "Update"
    h.click("general");
    h.click("Post");
    h.click("Message composer");
    h.type_text("Lunch is here");
    h.key(egui::Key::Enter);
    let p = all_posts(&h).pop().unwrap();
    assert_eq!(p.post_title.as_deref(), Some("Update"));
    assert_eq!(in_web(&h), before, "\"update\" isn't in what was written");
    assert_eq!(h.app.data.tasks.len(), tasks + 1, "the task workflow ran");
    assert!(h.app.data.tasks.iter().any(|t| t.name == "Lunch is here"));
    // a post of a file alone: no task named "Update"
    h.app.add_post(
        gen,
        "Update".into(),
        "Announcement",
        String::new(),
        vec![],
        &[],
    );
    assert!(
        !h.app.data.tasks.iter().any(|t| t.name == "Update"),
        "no task named after the default title"
    );
    // a typed title stands for the text, without its formatting marks (as
    // the iPhone gives it)
    h.app.add_post(
        gen,
        "**Update** on the move".into(),
        "Idea",
        String::new(),
        vec![],
        &[],
    );
    let last = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == web)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .text
        .clone();
    assert_eq!(
        last,
        format!("{} posted: Update on the move", h.app.who(h.app.data.me))
    );
}

/// Search doesn't list every post sent without a title for a word that is
/// part of the default title ("date", "Update"): nobody wrote it.
#[test]
fn search_leaves_out_the_default_post_title() {
    let mut h = posts_page();
    let gen = channel_id(&h, "general");
    add_post(
        &mut h,
        gen,
        2,
        "Update",
        "Lunch is here",
        "Announcement",
        60,
    );
    h.app.section = Section::Home;
    h.settle();
    h.click("Search");
    h.type_text("date");
    h.settle();
    assert!(!h.has("Lunch is here"), "not found by the default title");
    assert!(!h.has("Update"));
    h.key(egui::Key::Escape);
    h.click("Search");
    h.type_text("lunch");
    h.settle();
    assert!(h.has("Lunch is here"), "found by what was written");
    h.key(egui::Key::Escape);

    // nor finding in the channel
    h.click("general");
    h.app.channel_find = Some(String::new());
    h.settle();
    // the find box beside the bookmarks chip (a menu row has the same words)
    let row = h.rect("Add bookmarks, notes, and more").center().y;
    let find = h
        .painted_text("Find in channel")
        .into_iter()
        .map(|(r, _)| r.center())
        .find(|p| (p.y - row).abs() < 12.0)
        .unwrap();
    h.click_at(find);
    h.type_text("date");
    h.settle();
    assert_eq!(h.app.channel_find.as_deref(), Some("date"));
    assert!(
        h.painted_text("Lunch is here").is_empty(),
        "not in the channel's finds: {:?}",
        h.painted_text("Lunch is here")
    );
}

/// Logging out of the team (another workspace opens, with the sign-in
/// screen) starts the Posts page's channel, filters and @ picks over as
/// switching workspaces does.
#[test]
fn logging_out_starts_the_posts_page_channel_and_filters_over() {
    let mut h = posts_page();
    write_post(&mut h, "general", "Office move", "Monday");
    pick_filter(&mut h, "#  Channel", "# general");
    h.app.post.picked = vec![2];
    let addr = "127.0.0.1:1";
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-posts-out".into(),
        tag: 5,
        server: Some(addr.into()),
    });
    together::save_token(addr, "dev-posts-out", "token-1");
    h.click("Profile menu");
    h.click("Log out");
    h.click("Log out");
    assert!(h.app.data.extra.shared.is_none(), "logged out");
    assert_eq!(h.app.post.channel, 0);
    assert_eq!(h.app.post.f_channel, None);
    assert!(h.app.post.picked.is_empty());
}

/// "Open in channel" goes to the post in the channel, not the channel's
/// newest messages: the post is scrolled into sight, earlier messages
/// shown as far back as it is, and lit up for a moment. It used to open
/// the channel at the bottom, leaving the post to be searched for.
#[test]
fn open_in_channel_goes_to_the_post_and_lights_it_up() {
    let mut h = posts_page();
    let gen = channel_id(&h, "general");
    add_post(
        &mut h,
        gen,
        2,
        "Office move",
        "Boxes go Monday",
        "Announcement",
        90_000,
    );
    // many messages after it: further back than the channel shows at first
    let me = h.app.data.me;
    let c = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == gen)
        .unwrap();
    for i in 0..(crate::app::chat::CHAT_PAGE + 40) {
        let mut m = crate::app::slack::new_message(
            500_000 + i as u64,
            me,
            format!("later message {i}"),
            now_ts() - 80_000 + i as i64,
        );
        m.imported = None;
        c.messages.push(m);
    }
    h.settle();
    let open = h.rect("Open in channel");
    h.click_at(open.center());
    for _ in 0..6 {
        h.frame(vec![]);
    }
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.app.chat.open, Some(gen));
    let title = h.painted_text("Office move");
    let composer = h.rect("Message composer");
    assert!(
        title
            .iter()
            .any(|(r, _)| r.top() > 120.0 && r.bottom() < composer.top()),
        "the post is in sight: {title:?}, composer at {composer:?}"
    );
    let lit = |h: &Ui1| {
        let at = h.painted_text("Office move")[0].0;
        h.shapes.iter().any(|c| match &c.shape {
            egui::Shape::Rect(r) => {
                r.rect.contains(at.center())
                    && r.fill.a() > 0
                    && r.fill.r() as i32 > r.fill.b() as i32 + 20
            }
            _ => false,
        })
    };
    assert!(lit(&h), "lit up");
    // a few seconds later the light is gone
    for _ in 0..80 {
        h.frame(vec![]);
    }
    assert!(!lit(&h), "only for a moment");
    assert!(h.app.chat.flash.is_none());

    // going elsewhere before the light is out: it doesn't stay behind (the
    // other conversation keeps to its newest messages)
    h.app.section = Section::Posts;
    h.settle();
    h.click_at(h.rect("Open in channel").center());
    h.click("website-redesign");
    for _ in 0..80 {
        h.frame(vec![]);
    }
    assert!(h.app.chat.flash.is_none(), "the light is gone there too");
}

// ---------------------------------------------------------------- profiles

/// Alex's avatar circles (their colour) inside `area`: their radii.
fn alex_avatars(h: &Ui1, area: Rect) -> Vec<f32> {
    let alex = rgb(h.app.data.member(2).unwrap().color);
    let mut out = vec![];
    for c in &h.shapes {
        if let egui::Shape::Circle(s) = &c.shape {
            if s.fill == alex && area.contains(s.center) {
                out.push(s.radius);
            }
        }
    }
    out
}

/// The avatars beside messages in a conversation are a little smaller than
/// they were (radius 15).
#[test]
fn a_messages_avatar_is_a_little_smaller() {
    let mut h = posts_page();
    h.click("general");
    let list = Rect::from_min_max(pos2(330.0, 120.0), pos2(1440.0, 760.0));
    let r = alex_avatars(&h, list);
    assert!(!r.is_empty(), "Alex has a message in #general");
    assert!(r.iter().all(|r| *r <= 13.0 && *r >= 11.0), "{r:?}");
}

/// Clicking someone's name (or avatar) on a message opens their profile at
/// the right, where a thread shows: photo or initials, name, email, role,
/// and a button to message them. Escape or the close button closes it.
#[test]
fn a_name_on_a_message_opens_that_persons_profile() {
    let mut h = posts_page();
    h.click("general");
    let list = Rect::from_min_max(pos2(330.0, 120.0), pos2(1440.0, 760.0));
    let at = h.rect_in("Alex Kim", list).center();
    h.click_at(at);
    h.settle();
    assert!(h.has("Close profile"), "the profile opens");
    assert!(
        !h.painted_text("alex@example.com").is_empty(),
        "with the email"
    );
    assert!(!h.painted_text("Admin").is_empty(), "and the role");
    let panel = h.rect("Close profile");
    assert!(panel.left() > 900.0, "at the right: {panel:?}");
    h.key(egui::Key::Escape);
    h.settle();
    assert!(!h.has("Close profile"), "Escape closes it");

    // the avatar opens it too; Message opens a conversation with them
    let avatar = h
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Circle(s)
                if s.fill == rgb(h.app.data.member(2).unwrap().color)
                    && list.contains(s.center) =>
            {
                Some(s.center)
            }
            _ => None,
        })
        .unwrap();
    h.click_at(avatar);
    h.settle();
    assert!(h.has("Close profile"));
    h.click("Message Alex Kim");
    h.settle();
    let open = h.app.chat.open.unwrap();
    let c = h.app.data.channels.iter().find(|c| c.id == open).unwrap();
    assert_eq!(c.kind, ChannelKind::Direct);
    assert!(c.members.contains(&2), "a conversation with Alex");
    assert!(!h.has("Close profile"), "the conversation gets the room");
}

/// On the Posts page a post's author opens their profile at the right too;
/// my own profile offers Edit profile.
#[test]
fn a_posts_author_opens_their_profile_and_mine_can_be_edited() {
    let mut h = posts_page();
    let gen = channel_id(&h, "general");
    add_post(&mut h, gen, 2, "Office move", "Monday", "Announcement", 60);
    h.settle();
    let cards = Rect::from_min_max(pos2(330.0, 440.0), pos2(1440.0, 900.0));
    let at = h.rect_in("Alex Kim", cards).center();
    h.click_at(at);
    h.settle();
    assert_eq!(h.app.section, Section::Posts, "still on the Posts page");
    assert!(h.has("Close profile"));
    assert!(!h.painted_text("alex@example.com").is_empty());
    h.click("Close profile");
    h.settle();
    assert!(!h.has("Close profile"));

    write_post(&mut h, "general", "Lunch", "Pizza");
    h.click("Post");
    let me = h.app.who(h.app.data.me);
    let at = h.rect_in(&me, cards).center();
    h.click_at(at);
    h.settle();
    assert!(
        !h.has(&format!("Message {me}")),
        "no messaging myself from here"
    );
    h.click("Edit profile");
    assert!(matches!(h.app.modal, Some(Modal::Profile { .. })));
}

/// A post title with no spaces (a web address, say) wraps in the
/// conversation. It used to widen the whole conversation past the window,
/// so the message box and a message's hover toolbar were cut off at the
/// right.
#[test]
fn a_long_post_title_keeps_the_conversation_inside_the_window() {
    let mut h = Ui1::new();
    let gen = channel_id(&h, "general");
    let long = format!("https://example.com/{}", "a".repeat(140));
    add_post(&mut h, gen, 2, &long, "See the plan", "Idea", 60);
    h.click("general");
    h.settle();
    let composer = h.rect("Message composer");
    assert!(
        composer.right() < 1440.0,
        "the message box fits: {composer:?}"
    );
    let (title, _) = h.painted_text(&long)[0];
    assert!(
        title.height() > 30.0 && title.right() <= composer.right() + 20.0,
        "the title wraps inside: {title:?}"
    );
    let at = h.painted_text("See the plan")[0].0.center();
    h.hover(at);
    let more = h.rect("More actions");
    assert!(
        more.right() <= composer.right() + 20.0,
        "the toolbar fits: {more:?}"
    );
}
