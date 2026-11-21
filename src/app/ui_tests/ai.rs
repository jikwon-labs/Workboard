// Click-through tests for the AI tools: the AI page, conversation and
// thread summaries, writing in docs and task descriptions, and suggested
// subtasks. A real team server answers, with a stand-in for the AI
// (workboard_server::testing::FakeClaude) that keeps what it was sent.

use super::*;
use crate::app::ai::AiState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use workboard_server::testing::{FakeClaude, TestServer};

/// The seed workspace set up on a server that has an AI, signed in as its
/// owner, with the AI's tools showing.
fn on_ai_server(answer: &str) -> (Ui1, TestServer, Arc<FakeClaude>) {
    let fake = FakeClaude::new(answer);
    let server = TestServer::with_ai(fake.clone());
    let addr = format!("127.0.0.1:{}", server.port);
    let mut h = Ui1::new();
    h.app
        .connect_with(
            &addr,
            together::How::Claim {
                email: "owner@team.io".into(),
                password: "owner-pass-1".into(),
            },
        )
        .unwrap();
    until(&mut h, "the server's AI", |h| {
        h.app.connected() && h.app.ai_offered()
    });
    (h, server, fake)
}

/// Draws frames, taking in what the server says, until `cond` holds.
fn until(h: &mut Ui1, what: &str, cond: impl Fn(&Ui1) -> bool) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::time::Instant::now() < end {
        h.app.net_tick(None);
        h.frame(vec![]);
        if cond(h) {
            h.settle();
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!(
        "timed out waiting for: {} (status {:?}, toast {:?})",
        what, h.app.sync_rt.net_status, h.app.toast
    );
}

/// Whether `n` requests are answered.
fn answered(h: &Ui1, n: usize) -> bool {
    h.app
        .ai
        .jobs
        .values()
        .filter(|j| j.state == AiState::Done)
        .count()
        >= n
}

/// Clicks on `part` inside a painted text that also holds `within`.
fn click_text(h: &mut Ui1, within: &str, part: &str) {
    let mut at = None;
    each_text(&h.shapes, |ts, _| {
        let text = ts.galley.text();
        if at.is_some() || !text.contains(within) {
            return;
        }
        if let Some(b) = text.find(part) {
            let c = text[..b].chars().count() + part.chars().count() / 2;
            let r = ts
                .galley
                .pos_from_cursor(&ts.galley.from_ccursor(egui::text::CCursor::new(c)));
            at = Some(ts.pos + r.center().to_vec2());
        }
    });
    let at = at.unwrap_or_else(|| panic!("no {:?} painted in {:?}", part, within));
    h.click_at(at);
}

fn shown_containing(h: &Ui1, part: &str) -> Option<String> {
    drawn_texts(&h.shapes)
        .into_iter()
        .find(|t| t.contains(part))
}

/// Without a team server the AI page says where AI works and offers to
/// set one up; the box can't be typed in and there are no buttons that
/// would ask.
#[test]
fn without_a_server_the_ai_page_says_where_ai_works() {
    let mut h = Ui1::new();
    h.click("Rail AI");
    assert_eq!(h.app.section, Section::Ai);
    assert!(!h.painted_text("AI for this workspace").is_empty());
    assert!(h.has("Work together"));
    assert!(
        !h.has("What's due this week?"),
        "no questions to start from"
    );
    h.click("Ask AI");
    h.type_text("hello");
    assert_eq!(h.app.ai.draft, "", "the box takes nothing");
    h.click("Ask the AI");
    assert!(h.app.ai.chat.is_empty());
    // and none of the tools elsewhere
    h.click("Close AI");
    h.click("general");
    assert!(!h.has("Summarize conversation"));
}

/// On a server with AI the page greets with the box in the middle and
/// questions to start from. A question shows at once and its answer comes
/// in pieces: formatting shown as style, the task it cites as a link with
/// the task's name, which opens it. A follow-up takes the conversation
/// along; New chat starts over.
#[test]
fn the_ai_page_answers_about_the_workspace_and_links_what_it_cites() {
    let (mut h, _server, fake) = on_ai_server("");
    let (tid, tname) = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.parent.is_none() && !t.archived)
        .map(|t| (t.id, t.name.clone()))
        .unwrap();
    *fake.answer.lock().unwrap() = format!("**Short answer:** |it's [task:{}]|.", tid);
    h.click("Rail AI");
    assert!(!h.painted_text("What can I help with?").is_empty());
    assert!(h.has("What's due this week?") && h.has("Which tasks are overdue?"));
    assert!(
        shown_containing(&h, "test-model").is_some(),
        "the model shows"
    );
    h.click("Ask AI");
    h.type_text("Who is on the launch?");
    h.key(egui::Key::Enter);
    assert_eq!(h.app.ai.draft, "", "sent with Enter");
    assert_eq!(h.app.ai.chat.len(), 1);
    assert!(
        !h.painted_text("Who is on the launch?").is_empty(),
        "the question shows"
    );
    until(&mut h, "the answer", |h| answered(h, 1));
    let sent = fake.last_text();
    assert!(sent.contains("Question: Who is on the launch?"), "{sent}");
    assert!(sent.contains(&tname), "what the asker may see goes along");
    let shown = shown_containing(&h, "Short answer:").expect("the answer");
    assert_eq!(shown, format!("Short answer: it's {}.", tname));
    // the citation opens the task
    click_text(&mut h, "Short answer:", &tname);
    h.settle();
    assert_eq!(h.app.open_task, Some(tid));
    h.key(egui::Key::Escape);
    h.settle();
    assert_eq!(h.app.open_task, None);
    assert_eq!(h.app.section, Section::Ai, "still on the AI page");

    // a follow-up takes the first question and its answer along
    *fake.answer.lock().unwrap() = "Next Friday.".into();
    h.click("Ask AI");
    h.type_text("And when is it due?");
    h.click("Ask the AI");
    until(&mut h, "the second answer", |h| answered(h, 2));
    let asked = fake.asked();
    let msgs = asked.last().unwrap()["messages"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[0]["content"], "Who is on the launch?");
    assert_eq!(
        msgs[1]["content"],
        format!("**Short answer:** it's [task:{}].", tid)
    );
    assert!(shown_containing(&h, "Next Friday.").is_some());

    // a suggestion asks itself
    h.click("New chat");
    assert!(h.app.ai.chat.is_empty());
    assert!(!h.painted_text("What can I help with?").is_empty());
    h.click("Which tasks are overdue?");
    until(&mut h, "the suggestion's answer", |h| answered(h, 1));
    assert!(fake
        .last_text()
        .contains("Question: Which tasks are overdue?"));
}

/// A failed answer says why and can be asked again in its place.
#[test]
fn a_failed_answer_can_be_asked_again() {
    let (mut h, _server, fake) = on_ai_server("Fine now.");
    *fake.fail.lock().unwrap() = Some(workboard_server::ai::AI_FAILED.into());
    h.click("Rail AI");
    h.click("Ask AI");
    h.type_text("Anything new?");
    h.key(egui::Key::Enter);
    until(&mut h, "the failure", |h| {
        h.app
            .ai
            .jobs
            .values()
            .any(|j| matches!(j.state, AiState::Failed(_)))
    });
    assert!(!h.painted_text(workboard_server::ai::AI_FAILED).is_empty());
    *fake.fail.lock().unwrap() = None;
    // while the server's AI isn't there (just reconnected, before it says
    // so) there is no Try again, and the question stays
    h.app.ai_welcomed();
    h.settle();
    assert!(!h.has("Try again"));
    assert_eq!(h.app.ai.chat.len(), 1);
    h.app.ai_ready_now("test-model".into());
    h.settle();
    h.click("Try again");
    until(&mut h, "the answer", |h| answered(h, 1));
    assert_eq!(h.app.ai.chat.len(), 1, "asked again in its place");
    assert!(shown_containing(&h, "Fine now.").is_some());
}

/// "Summarize" in a conversation's header sums it up in a dialog; Copy
/// and Close. A summary closed before it is done is stopped on the server.
#[test]
fn a_conversation_is_summed_up_and_a_closed_summary_stops() {
    let (mut h, _server, fake) = on_ai_server("- Alex asked |about the launch");
    h.click("general");
    h.click("Summarize conversation");
    assert!(matches!(h.app.modal, Some(Modal::Ai(_))));
    until(&mut h, "the summary", |h| answered(h, 1));
    assert!(fake.last_text().contains("Conversation: #general"));
    assert!(
        shown_containing(&h, "Alex asked about the launch").is_some(),
        "{:?}",
        drawn_texts(&h.shapes)
    );
    h.click("Copy");
    assert_eq!(h.app.toast.as_ref().map(|t| t.0.as_str()), Some("Copied"));
    h.click("Close");
    assert!(h.app.modal.is_none());
    assert!(h.app.ai.jobs.is_empty(), "forgotten once nothing shows it");

    // closed in the middle: stopped
    fake.hold.store(true, Ordering::SeqCst);
    h.click("Summarize conversation");
    until(&mut h, "the first piece", |h| {
        h.app.ai.jobs.values().any(|j| !j.text.is_empty())
    });
    h.click("Stop");
    assert!(h.app.modal.is_none());
    until(&mut h, "the server to stop it", |_| {
        fake.stopped.load(Ordering::SeqCst) == 1
    });
    assert!(h.app.ai.jobs.is_empty());
}

/// A thread with replies has "Summarize thread" in its header; it sums up
/// that thread.
#[test]
fn a_thread_is_summed_up_from_its_header() {
    let (mut h, _server, fake) = on_ai_server("They will review after lunch.");
    let (cid, mid) = h
        .app
        .data
        .channels
        .iter()
        .find_map(|c| {
            c.messages
                .iter()
                .find(|m| !m.replies.is_empty())
                .map(|m| (c.id, m.id))
        })
        .expect("a thread in the seed");
    h.app.open_chat_at(cid, Some(mid));
    h.settle();
    h.click("Summarize thread");
    until(&mut h, "the summary", |h| answered(h, 1));
    let sent = fake.last_text();
    assert!(
        sent.contains("Looks great, I'll review them after lunch."),
        "{sent}"
    );
    assert!(shown_containing(&h, "They will review after lunch.").is_some());
}

/// The AI menu of a doc page works on the selected text, or on the whole
/// page when nothing is selected: the answer replaces the text or goes
/// below it, and the cursor goes to the end of it. When the text changed
/// while the AI worked, the answer isn't put in.
#[test]
fn doc_writing_with_ai_replaces_the_selection_or_goes_below() {
    let (mut h, _server, fake) = on_ai_server("Hello, team.");
    let id = h.new_doc_focused();
    h.type_text("helo team");
    cmd_key(&mut h, egui::Key::A);
    h.click("Doc AI");
    assert!(!h.painted_text("On the selected text").is_empty());
    h.click("AI Fix spelling and grammar");
    until(&mut h, "the fixed text", |h| answered(h, 1));
    let sent = fake.last_text();
    assert!(sent.contains("Fix the spelling") && sent.contains("<text>\nhelo team\n</text>"));
    h.click("Replace");
    assert!(h.app.modal.is_none());
    assert_eq!(h.doc_body(id), "Hello, team.");

    // nothing selected now: the whole page, the answer below it
    *fake.answer.lock().unwrap() = "Two short points".into();
    h.click("Doc AI");
    assert!(!h.painted_text("On the whole page").is_empty());
    h.click("AI Summarize");
    until(&mut h, "the summary", |h| answered(h, 1));
    h.click("Insert below");
    assert_eq!(h.doc_body(id), "Hello, team.\n\nTwo short points");

    // the page changes while the AI works: nothing is put in
    *fake.answer.lock().unwrap() = "Better words".into();
    h.click("Doc AI");
    h.click("AI Improve writing");
    until(&mut h, "the new words", |h| answered(h, 1));
    let d = h.app.data.docs.iter_mut().find(|d| d.id == id).unwrap();
    d.body = "Someone else wrote this".into();
    h.click("Replace");
    assert_eq!(h.doc_body(id), "Someone else wrote this");
    assert!(
        matches!(h.app.modal, Some(Modal::Ai(_))),
        "the answer stays to copy"
    );
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some(crate::app::ai::CHANGED)
    );
}

/// A new page with only a title offers a first draft from it.
#[test]
fn an_empty_page_offers_a_first_draft_from_its_title() {
    let (mut h, _server, fake) = on_ai_server("# Offsite\n- Pick a date");
    let id = h.new_doc_focused();
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == id)
        .unwrap()
        .title = "Team offsite".into();
    h.settle();
    h.click("Doc AI");
    assert!(!h.has("AI Improve writing"), "nothing to improve yet");
    h.click("AI Write a first draft");
    until(&mut h, "the draft", |h| answered(h, 1));
    assert!(fake.last_text().contains("Title: Team offsite"));
    h.click("Use this");
    assert_eq!(h.doc_body(id), "# Offsite\n- Pick a date");
}

/// In a task: "Suggest subtasks" lists the AI's suggestions, ticked; the
/// ticked ones become subtasks. "Write with AI" drafts a description from
/// the task's name when it has none.
#[test]
fn a_task_gets_suggested_subtasks_and_a_drafted_description() {
    let (mut h, _server, fake) =
        on_ai_server(r#"{"subtasks": ["Book a room", |"Send invites", "Order food"]}"#);
    let tid = h.open_clean_task();
    let name = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.id == tid)
        .unwrap()
        .name
        .clone();
    let before = h.app.subtasks(tid).len();
    h.click("Suggest subtasks");
    // the half-written answer isn't shown, only that it's thinking
    assert!(!h.painted_text("Thinking…").is_empty());
    until(&mut h, "the suggestions", |h| answered(h, 1));
    assert!(fake.last_text().contains(&format!("Task: {}", name)));
    assert_eq!(
        fake.asked().last().unwrap()["output_config"]["format"]["type"],
        "json_schema"
    );
    h.click("Subtask suggestion Send invites");
    h.click("Add subtasks");
    h.settle();
    let subs: Vec<String> = h.app.subtasks(tid).iter().map(|t| t.name.clone()).collect();
    assert_eq!(subs.len(), before + 2, "{subs:?}");
    assert!(subs.contains(&"Book a room".to_string()) && subs.contains(&"Order food".to_string()));
    assert!(!subs.contains(&"Send invites".to_string()), "unticked");
    assert_eq!(h.app.open_task, Some(tid), "the task stays open");

    // a description from the task's name
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == tid)
        .unwrap()
        .description
        .clear();
    *fake.answer.lock().unwrap() = "Goal: a room for everyone.".into();
    h.settle();
    h.click("Description AI");
    h.click("AI Write a first draft");
    until(&mut h, "the draft", |h| answered(h, 1));
    h.click("Use this");
    h.settle();
    assert_eq!(
        h.app
            .data
            .tasks
            .iter()
            .find(|t| t.id == tid)
            .unwrap()
            .description,
        "Goal: a room for everyone."
    );
}

/// When the connection drops while an answer comes, the answer says so
/// rather than waiting forever, and the tools stay while it comes back.
#[test]
fn an_answer_cut_off_by_the_connection_says_so() {
    let (mut h, server, fake) = on_ai_server("first |second");
    fake.hold.store(true, Ordering::SeqCst);
    h.click("Rail AI");
    h.click("Ask AI");
    h.type_text("Are you there?");
    h.key(egui::Key::Enter);
    until(&mut h, "the first piece", |h| {
        h.app.ai.jobs.values().any(|j| !j.text.is_empty())
    });
    drop(server);
    until(&mut h, "the answer to fail", |h| {
        h.app
            .ai
            .jobs
            .values()
            .any(|j| matches!(j.state, AiState::Failed(_)))
    });
    assert!(!h.painted_text(crate::app::ai::BROKE).is_empty());
    assert!(
        h.app.ai_offered(),
        "the page keeps its tools while reconnecting"
    );
    // nothing asked while away
    h.click("Ask AI");
    h.type_text("Hello?");
    h.key(egui::Key::Enter);
    assert!(
        h.app.ai.jobs.values().all(|j| j.state != AiState::Going),
        "not sent while away"
    );
}

/// Korean: the page and the dialogs read in Korean, and the request asks
/// for a Korean answer.
#[test]
fn the_ai_reads_and_answers_in_korean() {
    let (mut h, _server, fake) = on_ai_server("요약입니다");
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    h.click("Rail AI");
    assert!(!h.painted_text("무엇을 도와드릴까요?").is_empty());
    assert!(!h
        .painted_text("이 워크스페이스에 대해 무엇이든 물어보세요…")
        .is_empty());
    h.click("Close AI");
    h.click("general");
    h.click("Summarize conversation");
    until(&mut h, "the summary", |h| answered(h, 1));
    assert!(fake.last_text().contains("Write your answer in Korean."));
    assert!(!h
        .painted_text("AI의 답은 틀릴 수 있습니다. 쓰기 전에 확인하세요.")
        .is_empty());
    crate::i18n::set_lang(crate::i18n::Lang::En);
}

/// A message the answer cites links to its conversation, by name; a reply
/// opens in its thread. A citation of something this person doesn't have
/// is left out of the answer.
#[test]
fn a_cited_message_opens_in_its_conversation_and_thread() {
    let (mut h, _server, fake) = on_ai_server("");
    let (cid, parent, reply) = h
        .app
        .data
        .channels
        .iter()
        .find_map(|c| {
            c.messages
                .iter()
                .find(|m| !m.replies.is_empty())
                .map(|m| (c.id, m.id, m.replies[0].id))
        })
        .expect("a thread in the seed");
    *fake.answer.lock().unwrap() = format!(
        "Bora answered in a thread [msg:{}:{}]. Gone [task:1].",
        cid, reply
    );
    h.click("Rail AI");
    h.click("Ask AI");
    h.type_text("Who answered?");
    h.key(egui::Key::Enter);
    until(&mut h, "the answer", |h| answered(h, 1));
    let shown = shown_containing(&h, "Bora answered").expect("the answer");
    assert_eq!(shown, "Bora answered in a thread #general. Gone.");
    click_text(&mut h, "Bora answered", "#general");
    h.settle();
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.app.chat.open, Some(cid));
    assert_eq!(h.app.chat.thread_open, Some(parent), "the reply's thread");
}

/// The AI page's conversation belongs to its workspace: opening another
/// one leaves it behind, so it is neither shown there nor sent along with
/// that workspace's questions.
#[test]
fn another_workspace_never_sees_the_conversation() {
    let (mut h, _server, fake) = on_ai_server("A private answer.");
    h.click("Rail AI");
    h.click("Ask AI");
    h.type_text("What did the leads decide?");
    h.key(egui::Key::Enter);
    until(&mut h, "the answer", |h| answered(h, 1));
    assert!(!h.painted_text("What did the leads decide?").is_empty());
    // the same app opens another workspace (another file, another server)
    h.app.ws_file = "other-team.json".into();
    if let Some(c) = h.app.data.extra.shared.as_mut() {
        c.server = Some("127.0.0.1:1".into());
    }
    h.settle();
    assert!(h.app.ai.chat.is_empty() && h.app.ai.jobs.is_empty());
    assert!(h.app.ai.draft.is_empty());
    assert!(h.painted_text("What did the leads decide?").is_empty());
    assert_eq!(fake.asked().len(), 1);
}

/// On a computer the AI page opens with the cursor in its question box, so
/// a question can be typed straight away; Escape still leaves the page.
#[test]
fn the_ai_page_opens_with_the_cursor_in_its_box() {
    let (mut h, _server, _fake) = on_ai_server("");
    h.click("Rail AI");
    assert_eq!(h.app.section, Section::Ai);
    assert_eq!(
        h.ctx.memory(|m| m.focused()),
        Some(egui::Id::new(crate::app::ai::AI_BOX))
    );
    h.type_text("typed right away");
    assert_eq!(h.app.ai.draft, "typed right away");
    h.app.ai.draft.clear();
    h.key(egui::Key::Escape);
    h.settle();
    assert_ne!(h.app.section, Section::Ai, "Escape leaves the page");
}
