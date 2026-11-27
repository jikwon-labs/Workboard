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
    h.net_until("the server's AI", |h| {
        h.app.connected() && h.app.ai_offered()
    });
    (h, server, fake)
}

/// Whether `n` requests are answered: in a dialog, or questions of the
/// chat the AI page shows (kept in the chat once answered).
fn answered(h: &Ui1, n: usize) -> bool {
    let done = h
        .app
        .ai
        .jobs
        .values()
        .filter(|j| j.state == AiState::Done)
        .count();
    let kept = h
        .app
        .ai
        .open
        .and_then(|id| h.app.data.extra.ai_chats.iter().find(|c| c.id == id))
        .map_or(0, |c| c.turns.len());
    done + kept >= n
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
    h.texts().into_iter().find(|t| t.contains(part))
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
    h.net_until("the answer", |h| answered(h, 1));
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
    h.net_until("the second answer", |h| answered(h, 2));
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
    h.net_until("the suggestion's answer", |h| answered(h, 1));
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
    h.net_until("the failure", |h| {
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
    h.net_until("the answer", |h| answered(h, 1));
    assert!(h.app.ai.chat.is_empty(), "asked again in its place");
    assert_eq!(h.app.data.extra.ai_chats.len(), 1);
    assert_eq!(h.app.data.extra.ai_chats[0].turns.len(), 1);
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
    h.net_until("the summary", |h| answered(h, 1));
    assert!(fake.last_text().contains("Conversation: #general"));
    assert!(
        shown_containing(&h, "Alex asked about the launch").is_some(),
        "{:?}",
        h.texts()
    );
    h.click("Copy");
    assert_eq!(h.app.toast.as_ref().map(|t| t.0.as_str()), Some("Copied"));
    h.click("Close");
    assert!(h.app.modal.is_none());
    assert!(h.app.ai.jobs.is_empty(), "forgotten once nothing shows it");

    // closed in the middle: stopped
    fake.hold.store(true, Ordering::SeqCst);
    h.click("Summarize conversation");
    h.net_until("the first piece", |h| {
        h.app.ai.jobs.values().any(|j| !j.text.is_empty())
    });
    h.click("Stop");
    assert!(h.app.modal.is_none());
    h.net_until("the server to stop it", |_| {
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
    h.net_until("the summary", |h| answered(h, 1));
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
    h.net_until("the fixed text", |h| answered(h, 1));
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
    h.net_until("the summary", |h| answered(h, 1));
    h.click("Insert below");
    assert_eq!(h.doc_body(id), "Hello, team.\n\nTwo short points");

    // the page changes while the AI works: nothing is put in
    *fake.answer.lock().unwrap() = "Better words".into();
    h.click("Doc AI");
    h.click("AI Improve writing");
    h.net_until("the new words", |h| answered(h, 1));
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
    h.net_until("the draft", |h| answered(h, 1));
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
    h.net_until("the suggestions", |h| answered(h, 1));
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
    h.net_until("the draft", |h| answered(h, 1));
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
    h.net_until("the first piece", |h| {
        h.app.ai.jobs.values().any(|j| !j.text.is_empty())
    });
    drop(server);
    h.net_until("the answer to fail", |h| {
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
    h.net_until("the summary", |h| answered(h, 1));
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
    h.net_until("the answer", |h| answered(h, 1));
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
    h.net_until("the answer", |h| answered(h, 1));
    assert!(!h.painted_text("What did the leads decide?").is_empty());
    // the same app opens another workspace (another file, another server)
    h.app.ws_file = "other-team.json".into();
    if let Some(c) = h.app.data.extra.shared.as_mut() {
        c.server = Some("127.0.0.1:1".into());
    }
    // (the chat is kept in the first workspace's file, which the other
    // workspace doesn't read)
    h.app.data.extra.ai_chats.clear();
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

/// Hands this app's changes to the team server.
fn send(h: &mut Ui1) {
    h.app.sync_note_change();
    h.app.write_shared();
}

/// Takes in what the team server sends until `cond` holds, then draws.
fn receive(h: &mut Ui1, what: &str, cond: impl Fn(&App) -> bool) {
    crate::app::tests::net_until(&mut h.app, what, cond);
    h.settle();
}

/// The AI chats in the copy the team server keeps.
fn chats_on_server(server: &TestServer) -> Vec<u64> {
    let file = server.home.join(workboard_server::WORKSPACE_FILE);
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    v["extra"]["ai_chats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["id"].as_u64())
        .collect()
}

/// An answered question is kept as a chat in the sidebar and in the
/// workspace file (so it is there after a restart), but stays on this
/// computer: the server and the teammate never get it. Shared (after a
/// warning), the teammate finds it in their sidebar and reads it; what
/// they ask goes on in a chat of their own. Taken back, it leaves the
/// teammate's sidebar and stays with its owner. Only who asked shares or
/// deletes a chat.
#[test]
fn ai_chats_are_kept_on_this_computer_and_shared_only_when_asked() {
    let fake = FakeClaude::new("The launch is on Friday.");
    let server = TestServer::with_ai(fake.clone());
    let addr = server.addr();
    let mut a = Ui1::new();
    crate::app::tests::share_on(&mut a.app, &server);
    let code = crate::app::tests::invite_code_for_sam(&mut a.app, &server);
    a.net_until("the server's AI at A", |h| h.app.ai_offered());
    let mut b = Ui1::with_data(crate::app::tests::blank_as("Theirs", 1, "B"));
    b.app.connect_server(&addr, &code.to_lowercase()).unwrap();
    receive(&mut b, "B joined", |x| x.data.extra.shared.is_some());
    b.net_until("the server's AI at B", |h| h.app.ai_offered());

    // A asks; the answer is kept as a chat named after the question
    a.click("Rail AI");
    a.click("Ask AI");
    a.type_text("When is the launch?");
    a.key(egui::Key::Enter);
    a.net_until("the answer", |h| answered(h, 1));
    let chat = a.app.ai.open.expect("the chat shown");
    let c = a.app.ai_chat(chat).unwrap().clone();
    assert_eq!(c.title, "When is the launch?");
    assert_eq!(c.by, a.app.data.me);
    assert!(c.personal, "only on this computer");
    assert_eq!(c.turns[0].answer, "The launch is on Friday.");
    // in the workspace file, read back as it was
    let file = serde_json::to_string(&a.app.data).unwrap();
    let back: crate::model::Data = serde_json::from_str(&file).unwrap();
    assert_eq!(back.extra.ai_chats, a.app.data.extra.ai_chats);
    // a new chat, then the kept one from the sidebar
    a.click("New chat");
    assert_eq!(a.app.ai.open, None);
    assert!(shown_containing(&a, "The launch is on Friday.").is_none());
    a.click("AI chat When is the launch?");
    assert_eq!(a.app.ai.open, Some(chat));
    assert!(shown_containing(&a, "The launch is on Friday.").is_some());
    // never sent: not on the server, not at the teammate
    send(&mut a);
    crate::app::tests::net_until_saved(&mut a.app, &server.home);
    a.settle();
    assert!(!chats_on_server(&server).contains(&chat));
    b.settle();
    assert!(b.app.ai_chat(chat).is_none());
    assert!(!b.has("When is the launch?"), "not in B's sidebar");

    // sharing warns first; Cancel keeps it to A
    a.click("Share chat");
    assert!(matches!(a.app.modal, Some(Modal::ShareAiChat { .. })));
    assert!(shown_containing(&a, "only you can see").is_some());
    a.click("Cancel");
    assert!(a.app.ai_chat(chat).unwrap().personal);
    a.click("Share chat");
    a.click("Share");
    assert!(!a.app.ai_chat(chat).unwrap().personal);
    assert!(a.has("Stop sharing chat") && !a.has("Share chat"));
    send(&mut a);
    receive(&mut b, "the shared chat at B", |x| {
        x.ai_chat(chat).is_some()
    });
    assert!(
        workboard_server::testing::wait_until(
            std::time::Duration::from_secs(10),
            std::time::Duration::from_millis(50),
            || chats_on_server(&server).contains(&chat)
        ),
        "the server keeps the shared chat"
    );

    // B reads it from the sidebar, with who shared it; B can't share,
    // take back or delete it
    b.click("Rail AI");
    b.click("AI chat When is the launch?");
    assert_eq!(b.app.ai.open, Some(chat));
    assert!(shown_containing(&b, "The launch is on Friday.").is_some());
    assert!(shown_containing(&b, "shared this chat").is_some());
    assert!(!b.has("Share chat") && !b.has("Stop sharing chat") && !b.has("Delete chat"));
    // what B asks goes on in B's own chat, with A's turns along
    *fake.answer.lock().unwrap() = "Bora leads it.".into();
    b.click("Ask AI");
    b.type_text("Who leads it?");
    b.key(egui::Key::Enter);
    b.net_until("B's answer", |h| answered(h, 2));
    let mine = b.app.ai.open.unwrap();
    assert_ne!(mine, chat);
    let bc = b.app.ai_chat(mine).unwrap().clone();
    assert_eq!(bc.by, b.app.data.me);
    assert!(bc.personal);
    assert_eq!(bc.turns.len(), 2);
    let msgs = fake.asked().last().unwrap()["messages"].clone();
    assert_eq!(msgs[0]["content"], "When is the launch?");
    assert_eq!(
        b.app.ai_chat(chat).unwrap().turns.len(),
        1,
        "A's chat as it was"
    );
    // B's own chat doesn't reach A
    send(&mut b);
    crate::app::tests::net_until_saved(&mut b.app, &server.home);
    assert!(!chats_on_server(&server).contains(&mine));

    // A takes it back: it leaves B, and A keeps it as a private chat
    a.click("Stop sharing chat");
    let kept = a.app.ai.open.unwrap();
    assert_ne!(kept, chat);
    assert!(a.app.ai_chat(kept).unwrap().personal);
    assert_eq!(a.app.ai_chat(kept).unwrap().turns.len(), 1);
    send(&mut a);
    receive(&mut b, "the chat gone at B", |x| x.ai_chat(chat).is_none());
    assert!(b.app.ai_chat(mine).is_some(), "B's own chat stays");
    assert!(!chats_on_server(&server).contains(&kept));

    // A deletes it, after saying so
    a.click("Delete chat");
    assert!(
        matches!(a.app.modal, Some(Modal::ConfirmRemove { kind: "ai_chat", id, .. }) if id == kept),
        "{:?}",
        a.app.modal.is_some()
    );
    a.click("Delete");
    assert!(a.app.ai_chat(kept).is_none());
    assert_eq!(a.app.ai.open, None);
    assert!(!a.has("AI chat When is the launch?"));
}

/// The sidebar lists the newest few chats and "Show all" the rest.
#[test]
fn the_sidebar_lists_the_newest_ai_chats() {
    let mut h = Ui1::new();
    for i in 0..7u64 {
        let me = h.app.data.me;
        h.app.data.extra.ai_chats.push(crate::model::AiChat {
            id: 9000 + i,
            title: format!("Question {}", i),
            by: me,
            created: 100 + i as i64,
            turns: vec![],
            personal: true,
        });
    }
    h.settle();
    assert!(h.has("Question 6") && h.has("Question 2"));
    assert!(!h.has("Question 1"), "only the newest five");
    h.click("Show all");
    assert!(h.has("Question 1") && h.has("Question 0"));
    h.click("Show fewer");
    assert!(!h.has("Question 0"));
    h.click_side("Question 4", false);
    assert_eq!(h.app.section, Section::Ai);
    assert_eq!(h.app.ai.open, Some(9004));
}
