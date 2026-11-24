// Click-through tests for polls: "/poll" opens the window, choices are
// typed and added, the poll is posted, votes are cast, changed and taken
// back, and its author closes it.

use super::*;

fn last_message(h: &Ui1, cid: u64) -> Message {
    h.app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .clone()
}

/// "/poll <question>" in #general, then the choices typed in the window.
fn make_poll(h: &mut Ui1, question: &str, choices: &[&str], multi: bool) -> u64 {
    h.click("general");
    h.send_message(&format!("/poll {}", question));
    assert!(
        matches!(h.app.modal, Some(Modal::NewPoll(_))),
        "the New poll window"
    );
    h.settle();
    assert!(h.has("Poll question") && h.has("Choice 1") && h.has("Choice 2"));
    for (i, c) in choices.iter().enumerate() {
        if i >= 2 {
            h.click("Add a choice");
        }
        h.click(&format!("Choice {}", i + 1));
        h.type_text(c);
    }
    if multi {
        h.click("Allow more than one choice");
    }
    h.click("Create poll");
    assert!(h.app.modal.is_none(), "posted");
    h.app.chat.open.unwrap()
}

#[test]
fn make_a_poll_vote_change_and_close_it() {
    let mut h = Ui1::new();
    let cid = make_poll(&mut h, "Lunch?", &["Pizza", "Noodles", "Salad"], false);
    let m = last_message(&h, cid);
    let p = m.poll.clone().expect("a poll");
    assert_eq!(p.question, "Lunch?");
    assert_eq!(p.options, vec!["Pizza", "Noodles", "Salad"]);
    assert!(!p.multi && !p.anonymous && p.closed.is_none());
    // what apps from before polls show
    assert_eq!(m.text, "Lunch?\n1. Pizza\n2. Noodles\n3. Salad");
    h.settle();
    assert!(h.has("Vote Pizza") && h.has("Vote Salad"));
    assert!(h.texts().iter().any(|t| t == "Pick one"));
    // one choice at a time: picking another moves the vote
    let me = h.app.data.me;
    h.click("Vote Pizza");
    assert_eq!(last_message(&h, cid).votes.get(&me), Some(&vec![0]));
    h.click("Vote Salad");
    assert_eq!(last_message(&h, cid).votes.get(&me), Some(&vec![2]));
    assert!(h.texts().iter().any(|t| t == "1 person voted"));
    // the same one again takes it back
    h.click("Vote Salad");
    assert!(last_message(&h, cid).votes.is_empty());
    h.click("Vote Noodles");
    // the author closes it: no more votes
    h.click("Close poll");
    assert!(last_message(&h, cid).poll.unwrap().closed.is_some());
    h.settle();
    assert!(h.texts().iter().any(|t| t.contains("Poll closed")));
    h.click("Vote Pizza");
    assert_eq!(last_message(&h, cid).votes.get(&me), Some(&vec![1]));
    h.click("Reopen poll");
    assert!(last_message(&h, cid).poll.unwrap().closed.is_none());
}

#[test]
fn a_poll_can_take_several_choices_and_up_to_ten() {
    let mut h = Ui1::new();
    let cid = make_poll(&mut h, "Which days?", &["Mon", "Tue", "Wed"], true);
    let me = h.app.data.me;
    h.settle();
    assert!(h.texts().iter().any(|t| t == "Pick one or more"));
    h.click("Vote Wed");
    h.click("Vote Mon");
    assert_eq!(last_message(&h, cid).votes.get(&me), Some(&vec![0, 2]));
    // the window takes ten choices at most, and Create needs two filled
    h.send_message("/poll Ten?");
    for _ in 0..12 {
        if h.has("Add a choice") {
            h.click("Add a choice");
        }
    }
    let Some(Modal::NewPoll(d)) = &h.app.modal else {
        panic!("the window");
    };
    assert_eq!(d.options.len(), 10);
    assert_eq!(d.question, "Ten?");
    h.click("Remove choice 10");
    let Some(Modal::NewPoll(d)) = &h.app.modal else {
        panic!("the window");
    };
    assert_eq!(d.options.len(), 9);
    h.click("Create poll");
    assert!(
        matches!(h.app.modal, Some(Modal::NewPoll(_))),
        "nothing to post without two choices"
    );
}

#[test]
fn an_anonymous_poll_does_not_say_who_voted() {
    let mut h = Ui1::new();
    h.click("general");
    h.send_message("/poll Secret?");
    h.click("Choice 1");
    h.type_text("Yes");
    h.click("Choice 2");
    h.type_text("No");
    h.click("Anonymous votes");
    h.click("Create poll");
    let cid = h.app.chat.open.unwrap();
    assert!(last_message(&h, cid).poll.unwrap().anonymous);
    h.click("Vote Yes");
    h.settle();
    assert!(h.texts().iter().any(|t| t.contains("Anonymous")));
    // someone else's message: no Close poll for me
    let mine = h.count_named("Close poll");
    assert!(mine >= 1);
    let me = h.app.data.me;
    let mut m = last_message(&h, cid);
    m.id = h.app.data.new_id();
    m.author = h.app.data.members.iter().find(|x| x.id != me).unwrap().id;
    m.votes.clear();
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .push(m);
    h.settle();
    assert!(h.has("Vote Yes"));
    assert_eq!(h.count_named("Close poll"), mine, "only on my own poll");
}
