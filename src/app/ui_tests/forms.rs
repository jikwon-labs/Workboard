// Click tests for the Forms pages: the hub's sidebar and table, the
// templates, the icon picker, the question editor, filling a form out and
// its Answers.

use super::*;
use crate::app::more::FormTab;

fn form(h: &Ui1, id: u64) -> Form {
    h.app
        .data
        .extra
        .forms
        .iter()
        .find(|f| f.id == id)
        .cloned()
        .expect("the form is there")
}

fn form_named(h: &Ui1, name: &str) -> u64 {
    h.app
        .data
        .extra
        .forms
        .iter()
        .find(|f| f.name == name)
        .map(|f| f.id)
        .expect("a form with that name")
}

fn open_forms(h: &mut Ui1) {
    h.click("Rail More");
    h.click("Forms");
}

/// A form just made is in the sidebar while it is open, the toast says
/// where it went, and the table shows it (marked) once back on the list.
#[test]
fn a_new_form_is_listed_at_once() {
    let mut h = Ui1::new();
    open_forms(&mut h);
    h.click("New form");
    let fid = h.app.form_open.expect("the new form opens");
    let name = form(&h, fid).name;
    assert!(
        h.has(&format!("Open form {}", name)),
        "the sidebar lists it while it is open"
    );
    assert!(h.app.toast.as_ref().is_some_and(|(m, _)| m.contains(&name)));
    assert!(form(&h, fid).updated > 0);
    h.click("Back to all forms");
    assert!(h.app.form_open.is_none());
    assert!(h.has(&format!("Form {}", name)), "and the table");
    assert!(!h.painted_text("Just made").is_empty());
    // a row opens its form; its Fill out opens that tab
    h.click(&format!("Fill out {}", name));
    assert_eq!(h.app.form_open, Some(fid));
    assert_eq!(h.app.form_tab, FormTab::Fill);
}

/// All / My / Favorites narrow the table, and the sidebar's search narrows
/// both the sidebar and the table.
#[test]
fn form_filters_and_search() {
    let mut h = Ui1::new();
    open_forms(&mut h);
    h.app.new_form_from(Some(0));
    h.app.new_form_from(Some(4));
    let fb = form_named(&h, "Feedback Form");
    let it = form_named(&h, "IT Requests");
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != h.app.data.me)
        .map(|m| m.id)
        .expect("a teammate");
    h.app
        .data
        .extra
        .forms
        .iter_mut()
        .find(|f| f.id == it)
        .unwrap()
        .owner = other;
    h.app.form_open = None;
    h.settle();
    assert!(h.has("Form Feedback Form") && h.has("Form IT Requests"));
    h.click("My forms");
    assert!(h.has("Form Feedback Form") && !h.has("Form IT Requests"));
    h.click("Favorites");
    assert!(!h.has("Form Feedback Form"));
    h.click("All forms");
    h.click(&format!("Star form:{}", fb));
    h.click("Favorites");
    assert!(h.has("Form Feedback Form") && !h.has("Form IT Requests"));
    h.click("All forms");
    h.click("Find a form");
    h.type_text("IT");
    h.settle();
    assert!(h.has("Open form IT Requests") && !h.has("Open form Feedback Form"));
    assert!(h.has("Form IT Requests") && !h.has("Form Feedback Form"));
}

/// A template makes a form with its questions and tile; the icon picker,
/// from the editor or the table, changes the tile's colour and icon.
#[test]
fn template_and_icon_picker() {
    let mut h = Ui1::new();
    open_forms(&mut h);
    h.click("IT Requests");
    let fid = h.app.form_open.expect("the form opens");
    let f = form(&h, fid);
    assert_eq!((f.icon.as_str(), f.color.as_str()), ("settings", "blue"));
    assert_eq!(f.fields.len(), 3);
    h.click("Change icon");
    h.click("Color teal");
    h.click("Icon heart");
    let f = form(&h, fid);
    assert_eq!((f.icon.as_str(), f.color.as_str()), ("heart", "teal"));
    h.key(egui::Key::Escape);
    h.click("Back to all forms");
    h.click("Change icon IT Requests");
    h.click("Icon bolt");
    assert_eq!(form(&h, fid).icon, "bolt");
    assert_eq!(h.app.form_open, None, "the tile doesn't open the form");
}

/// Add question offers every kind but those a form has one of when one is
/// there already; a question's kind changes from its own menu.
#[test]
fn question_kinds_from_the_menus() {
    let mut h = Ui1::new();
    h.app.new_form();
    h.settle();
    let fid = h.app.form_open.unwrap();
    h.click("Add question");
    assert!(
        !h.painted_text("In use").is_empty(),
        "a second task name is not offered"
    );
    h.click("Kind Due date");
    let f = form(&h, fid);
    assert_eq!(f.fields.len(), 3);
    assert_eq!(f.fields[2].map, "due");
    h.click("Question 2 kind");
    h.click("Kind Dropdown");
    assert_eq!(form(&h, fid).fields[1].map, "dropdown");
}

/// A dropdown's choices are typed in: Enter adds them (several at once
/// split at commas), a repeat is refused with a line saying so, × removes one.
#[test]
fn dropdown_choices_are_typed_in() {
    let mut h = Ui1::new();
    h.app.new_form();
    h.settle();
    let fid = h.app.form_open.unwrap();
    h.click("Question 2 kind");
    h.click("Kind Dropdown");
    h.click("Add choice 2");
    h.type_text("Red, Blue");
    h.key(egui::Key::Enter);
    assert_eq!(form(&h, fid).fields[1].options, ["Red", "Blue"]);
    // the field stays for the next one
    h.type_text("Red");
    h.key(egui::Key::Enter);
    assert_eq!(form(&h, fid).fields[1].options.len(), 2);
    assert!(!h.painted_text("\"Red\" is already a choice.").is_empty());
    h.click("Remove choice Blue");
    assert_eq!(form(&h, fid).fields[1].options, ["Red"]);
}

/// A question dragged by its handle goes where it is dropped; its menu
/// copies and deletes it.
#[test]
fn questions_drag_copy_and_delete() {
    let mut h = Ui1::new();
    h.app.new_form();
    h.settle();
    let fid = h.app.form_open.unwrap();
    h.drag("Drag question 2", "Drag question 1");
    assert_eq!(form(&h, fid).fields[0].label, "Details");
    h.click("Question 2 menu");
    h.click("Duplicate question 2");
    let f = form(&h, fid);
    assert_eq!(f.fields.len(), 3);
    assert_eq!(f.fields[2].label, "Title");
    assert_eq!(
        f.fields[2].map, "text",
        "a copy of the task name question becomes short text"
    );
    h.click("Question 3 menu");
    h.click("Delete question 3");
    assert_eq!(form(&h, fid).fields.len(), 2);
}

/// Submit with a required question left empty says so under that question;
/// answered, it makes a task that the form's Answers tab lists, as it does
/// an answer from before tasks kept their form.
#[test]
fn fill_out_and_see_the_answers() {
    let mut h = Ui1::new();
    open_forms(&mut h);
    h.click("IT Requests");
    let fid = h.app.form_open.unwrap();
    h.click("Required 1");
    h.click("Fill out");
    let n = h.app.data.tasks.len();
    h.click("Submit");
    assert_eq!(h.app.data.tasks.len(), n);
    assert_eq!(h.app.sweep.forms.error_at, Some(0));
    assert!(!h.painted_text("Please answer \"Issue\".").is_empty());
    h.click("Answer 1");
    h.type_text("Printer jams");
    h.click("Answer 2 High");
    h.click("Submit");
    let t = task_named(&h, "Printer jams").clone();
    assert_eq!(t.form, Some(fid));
    assert_eq!(t.priority, 2);
    assert_eq!(h.app.sweep.forms.error_at, None);
    h.click("Answers");
    assert!(h.has("Answer task Printer jams"));
    let list = form(&h, fid).list_id;
    h.app.pending.log = Some("submitted the form IT Requests".into());
    h.app.apply(Action::Add {
        list,
        name: "Older answer".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    h.settle();
    assert!(h.has("Answer task Older answer"));
    h.click("Answer task Printer jams");
    assert_eq!(h.app.open_task, Some(t.id));
}

/// A form with no List says so in the table; the warning opens the form,
/// where the List is picked.
#[test]
fn a_form_without_a_list_gets_one() {
    let mut h = Ui1::new();
    open_forms(&mut h);
    h.app.new_form();
    let fid = h.app.form_open.unwrap();
    let name = form(&h, fid).name;
    h.app
        .data
        .extra
        .forms
        .iter_mut()
        .find(|f| f.id == fid)
        .unwrap()
        .list_id = 0;
    h.app.form_open = None;
    h.settle();
    h.click(&format!("Choose a List for {}", name));
    assert_eq!(h.app.form_open, Some(fid));
    let (list, path) = h.app.all_list_paths()[0].clone();
    h.click("Pick the List for new tasks");
    h.click(&path);
    assert_eq!(form(&h, fid).list_id, list);
}

/// The hubs share one filter value (Docs' Archived is 6, a team's page 9).
/// Coming back to Forms from another hub other than by its own button (a
/// search result, Back) starts on All forms, not on whatever the other
/// hub's filter would mean here (Favorites, empty).
#[test]
fn forms_dont_take_another_hubs_filter() {
    let mut h = Ui1::new();
    open_forms(&mut h);
    h.click("Feedback Form");
    h.click("Back to all forms");
    assert!(h.has("Form Feedback Form"));
    // Docs' Archived, then the form from the search
    h.click("Rail More");
    h.click("Docs");
    h.click("Archived");
    assert_eq!(h.app.section, Section::Docs);
    h.click("Search");
    h.type_text("Feedback");
    h.click("Feedback Form");
    assert_eq!(h.app.section, Section::Forms);
    h.click("Back to all forms");
    assert!(h.has("Form Feedback Form"), "All forms, not Favorites");
    assert!(!h.painted_text("All forms").is_empty());
    // and with Back
    h.click("Rail More");
    h.click("Docs");
    h.click("Archived");
    h.click("Go back");
    assert_eq!(h.app.section, Section::Forms);
    assert!(h.has("Form Feedback Form"), "All forms after Back too");
    // the hub's own filter still holds while it is shown
    h.click("Favorites");
    assert!(!h.has("Form Feedback Form"));
}

/// A blank form (Title, Details) open on its Edit tab, the pointer on the
/// window as it is for a person (the harness drops a first click otherwise).
fn blank_form(h: &mut Ui1) -> u64 {
    h.app.new_form();
    h.settle();
    h.hover(pos2(700.0, 450.0));
    h.app.form_open.expect("the new form opens")
}

/// Answers typed on Fill out stay with their question when the questions
/// change on Edit: moved up, deleted. Before, they stayed at their place in
/// the list, so the task name went into the description and the other way.
#[test]
fn answers_stay_with_their_question_when_questions_move() {
    let mut h = Ui1::new();
    let fid = blank_form(&mut h);
    h.click("Fill out");
    h.click("Answer 1");
    h.type_text("Desk lamp");
    h.click("Answer 2");
    h.type_text("Flickers at night");
    h.click("Edit");
    h.click("Question 2 menu");
    h.click("Move question 2 up");
    assert_eq!(form(&h, fid).fields[0].map, "description");
    h.click("Fill out");
    h.click("Submit");
    let t = task_named(&h, "Desk lamp");
    assert_eq!(t.description, "Flickers at night");
    // deleting the first question (now Details) leaves the name's answer
    // with the name question
    h.click("Answer 1");
    h.type_text("Too dim");
    h.click("Answer 2");
    h.type_text("Lamp two");
    h.click("Edit");
    h.click("Question 1 menu");
    h.click("Delete question 1");
    assert_eq!(form(&h, fid).fields.len(), 1);
    h.click("Fill out");
    h.click("Submit");
    assert_eq!(task_named(&h, "Lamp two").description, "");
    assert!(!h.app.data.tasks.iter().any(|t| t.name == "Too dim"));
}

/// The same when a teammate turns the questions around while this person
/// is filling the form out.
#[test]
fn answers_stay_with_their_question_when_a_teammate_moves_it() {
    let mut h = Ui1::new();
    let fid = blank_form(&mut h);
    h.click("Fill out");
    h.click("Answer 1");
    h.type_text("Chair");
    h.click("Answer 2");
    h.type_text("Wobbly leg");
    let f = h
        .app
        .data
        .extra
        .forms
        .iter_mut()
        .find(|f| f.id == fid)
        .unwrap();
    f.fields.reverse();
    h.settle();
    h.click("Submit");
    assert_eq!(task_named(&h, "Chair").description, "Wobbly leg");
}

/// A question's kind changed on Edit drops what was typed for it (a
/// checkbox doesn't show the old text but would have sent it); a problem
/// found on Submit moves with its question.
#[test]
fn a_kind_change_drops_the_answer_and_the_problem_moves_along() {
    let mut h = Ui1::new();
    let fid = blank_form(&mut h);
    h.click("Fill out");
    h.click("Answer 2");
    h.type_text("hello");
    h.click("Edit");
    h.click("Question 2 kind");
    h.click("Kind Checkbox");
    assert_eq!(form(&h, fid).fields[1].map, "checkbox");
    h.click("Fill out");
    h.click("Answer 1");
    h.type_text("Checked nothing");
    h.click("Submit");
    assert_eq!(task_named(&h, "Checked nothing").description, "");
    // the problem with the first question, which then moves down
    h.click("Edit");
    h.click("Required 1");
    h.click("Fill out");
    h.click("Submit");
    assert_eq!(h.app.sweep.forms.error_at, Some(0));
    h.click("Edit");
    h.click("Question 1 menu");
    h.click("Move question 1 down");
    h.click("Fill out");
    assert_eq!(h.app.sweep.forms.error_at, Some(1), "still under Title");
    assert!(!h.painted_text("Please answer \"Title\".").is_empty());
    // answered, it goes away
    h.click("Answer 2");
    h.type_text("Lamp");
    h.click("Submit");
    assert_eq!(h.app.sweep.forms.error_at, None);
    assert!(h.painted_text("Please answer \"Title\".").is_empty());
}

/// A choice picked on Fill out and then removed on Edit is no longer the
/// answer: the dropdown asks again, and Submit doesn't send it.
#[test]
fn a_removed_choice_is_no_longer_the_answer() {
    let mut h = Ui1::new();
    let fid = blank_form(&mut h);
    h.click("Question 2 kind");
    h.click("Kind Dropdown");
    h.click("Add choice 2");
    h.type_text("Red, Blue");
    h.key(egui::Key::Enter);
    h.click("Fill out");
    h.click("Answer 1");
    h.type_text("Paint the wall");
    h.click("Answer 2");
    h.click("Choice Blue");
    assert_eq!(h.app.form_fill.get(&1).map(String::as_str), Some("Blue"));
    h.click("Edit");
    h.click("Remove choice Blue");
    assert_eq!(form(&h, fid).fields[1].options, ["Red"]);
    h.click("Fill out");
    assert!(
        !h.painted_text("Choose").is_empty(),
        "the dropdown asks again"
    );
    h.click("Submit");
    assert_eq!(task_named(&h, "Paint the wall").description, "");
}

/// A submit with the name question left empty names the task after the
/// form in the language of the person submitting, like any new task.
#[test]
fn an_unnamed_answer_is_named_in_the_interface_language() {
    let mut h = Ui1::new();
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    blank_form(&mut h);
    h.click("Fill out");
    h.click("Submit");
    let name = h.app.data.tasks.last().map(|t| t.name.clone());
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert_eq!(name.as_deref(), Some("새 양식 답변"));
    // someone using it in English (the form keeps the name it was made with)
    h.click("Submit");
    assert_eq!(
        h.app.data.tasks.last().map(|t| t.name.as_str()),
        Some("새 양식 submission")
    );
}

/// A ticked Checkbox question goes into the task description as the box
/// read on the screen it was ticked on: 예 in Korean, not Yes.
#[test]
fn a_ticked_checkbox_is_written_in_the_interface_language() {
    let mut h = Ui1::new();
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    blank_form(&mut h);
    h.click("Question 2 kind");
    h.click("Kind Checkbox");
    h.click("Fill out");
    h.click("Answer 1");
    h.type_text("Agreed");
    h.click("Answer 2");
    h.click("Submit");
    let d = task_named(&h, "Agreed").description.clone();
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert_eq!(d, "자세한 내용: 예");
}

/// A form whose name was cleared shows as "Untitled form" in the sidebar,
/// the table and its header (as an unnamed doc shows "Untitled"), not as
/// an empty row nobody can tell apart.
#[test]
fn a_form_without_a_name_shows_as_untitled() {
    let mut h = Ui1::new();
    let fid = blank_form(&mut h);
    h.click("Form name");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.key(egui::Key::Backspace);
    h.settle();
    assert_eq!(form(&h, fid).name, "");
    assert!(h.has("Open form Untitled form"), "the sidebar");
    assert!(
        h.painted_text("Untitled form").len() >= 2,
        "the sidebar and the header"
    );
    h.click("Back to all forms");
    assert!(h.has("Form Untitled form"), "the table");
    h.click("Find a form");
    h.type_text("untitled");
    assert!(h.has("Form Untitled form"), "found by what it shows");
}

/// Counts read right with one: "1 form", "1 answer", as "1 member" does.
#[test]
fn one_form_and_one_answer_are_counted_in_the_singular() {
    let mut h = Ui1::new();
    open_forms(&mut h);
    h.click("IT Requests");
    h.click("Fill out");
    h.click("Answer 1");
    h.type_text("Screen flickers");
    h.click("Submit");
    h.click("Answers");
    assert!(!h.painted_text("1 answer").is_empty());
    assert!(h.painted_text("1 answers").is_empty());
    h.click("Back to all forms");
    assert!(!h.painted_text("1 form").is_empty());
    assert!(h.painted_text("1 forms").is_empty());
}

/// A dropdown question with no choices yet: the editor says so, its menu on
/// Fill out says so instead of opening empty, and, as nobody can answer it,
/// it doesn't stop Submit even when it is required.
#[test]
fn a_dropdown_without_choices_says_so_and_doesnt_block_submit() {
    let mut h = Ui1::new();
    blank_form(&mut h);
    h.click("Question 2 kind");
    h.click("Kind Dropdown");
    h.click("Required 2");
    assert!(!h
        .painted_text("No choices yet. People filling the form out can't pick anything.")
        .is_empty());
    h.click("Fill out");
    h.click("Answer 2");
    assert!(!h.painted_text("No choices yet").is_empty());
    h.key(egui::Key::Escape);
    h.click("Answer 1");
    h.type_text("Nothing to pick");
    h.click("Submit");
    assert_eq!(h.app.sweep.forms.error_at, None);
    task_named(&h, "Nothing to pick");
}

/// Presses Delete in the "Delete the form?" dialog (its title and the
/// dialog are named Delete too: the button is the lowest small one).
fn confirm_delete(h: &mut Ui1) {
    let at = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Delete" && r.height() < 40.0)
        .map(|(_, r)| r.center())
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .expect("the dialog's Delete button");
    h.click_at(at);
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

/// Two people on a team server, each clicking in their own app: a form the
/// owner makes reaches the teammate, whose answer comes back to the owner's
/// Answers tab with a notice; a rename reaches the teammate; two edits at
/// once to different parts of one form both stay; a delete takes the form
/// the teammate has open back to the list; a form for the owner's Personal
/// space never reaches the teammate.
#[test]
fn forms_go_between_teammates_through_a_server() {
    let server = workboard_server::testing::TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);
    let mut a = Ui1::new();
    a.app.connect_server(&addr, "").unwrap();
    receive(&mut a, "A on the server", |x| x.data.extra.shared.is_some());
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !server.home.join(workboard_server::WORKSPACE_FILE).is_file()
        && std::time::Instant::now() < until
    {
        a.app.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    receive(&mut a, "the people list", |x| {
        x.sync_rt.people.iter().any(|p| p.member == 3)
    });
    a.app.access_request(wb_net::Msg::CreateInvite {
        member: 3,
        role: wb_net::Role::Member,
    });
    receive(&mut a, "an invite code", |x| {
        x.sync_rt
            .people
            .iter()
            .any(|p| p.member == 3 && p.invite.is_some())
    });
    let code = a
        .app
        .sync_rt
        .people
        .iter()
        .find(|p| p.member == 3)
        .and_then(|p| p.invite.clone())
        .unwrap();
    let mut b = Ui1::new();
    b.app = App::build(None, Some(crate::app::tests::blank_as("Theirs", 1, "B")));
    b.app.notify_rt.last_clock = i64::MAX;
    b.app.connect_server(&addr, &code.to_lowercase()).unwrap();
    receive(&mut b, "B joined", |x| x.data.extra.shared.is_some());
    assert_eq!(b.app.data.me, 3);

    // the owner makes a form; the teammate finds it in the sidebar
    open_forms(&mut a);
    a.click("IT Requests");
    let fid = a.app.form_open.unwrap();
    a.app.notify_scan(); // (the first look only remembers what is there)
    send(&mut a);
    receive(&mut b, "the form at B", |x| {
        x.data.extra.forms.iter().any(|f| f.id == fid)
    });
    open_forms(&mut b);
    b.click("Open form IT Requests");
    assert_eq!(b.app.form_open, Some(fid));

    // the teammate answers; the owner sees the answer and is told
    b.click("Fill out");
    b.click("Answer 1");
    b.type_text("Printer jams");
    b.click("Answer 2 Urgent");
    b.click("Submit");
    send(&mut b);
    receive(&mut a, "B's answer at A", |x| {
        x.data
            .tasks
            .iter()
            .any(|t| t.form == Some(fid) && t.name == "Printer jams")
    });
    a.click("Answers");
    assert!(a.has("Answer task Printer jams"));
    assert!(!a.painted_text("1 answer").is_empty());
    a.app.notify_scan();
    assert!(
        a.app
            .notify_rt
            .sent
            .iter()
            .any(|n| n.title == "New answer to IT Requests"),
        "the owner is told: {:?}",
        a.app.notify_rt.sent
    );

    // a rename reaches the teammate
    a.click("Edit");
    a.click("Form name");
    a.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    a.type_text("Help desk");
    send(&mut a);
    receive(&mut b, "the new name at B", |x| {
        x.data.extra.forms.iter().any(|f| f.name == "Help desk")
    });
    assert!(b.has("Open form Help desk"));

    // both change the form at once, different parts of it
    a.click("Form description");
    a.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    a.type_text("Tell us what broke.");
    send(&mut a);
    b.click("Edit");
    b.click("Change icon");
    b.click("Icon heart");
    b.key(egui::Key::Escape);
    send(&mut b);
    let both = |x: &App| {
        x.data
            .extra
            .forms
            .iter()
            .any(|f| f.id == fid && f.icon == "heart" && f.description == "Tell us what broke.")
    };
    receive(&mut a, "both changes at A", both);
    receive(&mut b, "both changes at B", both);

    // the owner deletes it while the teammate has it open
    a.click("Back to all forms");
    a.click("Form menu Help desk");
    a.click("Delete form");
    confirm_delete(&mut a);
    assert!(a.app.data.extra.forms.iter().all(|f| f.id != fid));
    send(&mut a);
    receive(&mut b, "the delete at B", |x| {
        x.data.extra.forms.iter().all(|f| f.id != fid)
    });
    assert_eq!(b.app.form_open, None, "back to the list");
    assert!(!b.has("Open form Help desk"));
    assert!(
        b.app.data.tasks.iter().any(|t| t.name == "Printer jams"),
        "the tasks it made stay"
    );

    // a form for the owner's Personal space stays the owner's
    a.app.create_personal_list();
    a.settle();
    a.click("New form");
    let private = a.app.form_open.unwrap();
    a.click("Pick the List for new tasks");
    a.click("Personal / Personal List");
    send(&mut a);
    let list = a.app.data.first_list().unwrap();
    a.app.apply(Action::Add {
        list,
        name: "After the private form".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    send(&mut a);
    receive(&mut b, "A's later task at B", |x| {
        x.data.tasks.iter().any(|t| t.name == "After the private form")
    });
    assert!(b.app.data.extra.forms.iter().all(|f| f.id != private));
}

/// The Answers tab's filter belongs to the form it was picked on: another
/// form's Answers start on All, as its typed answers and problems start
/// empty.
#[test]
fn another_forms_answers_start_on_all() {
    let mut h = Ui1::new();
    open_forms(&mut h);
    for (tpl, answer) in [("Feedback Form", "Loved it"), ("IT Requests", "No wifi")] {
        h.click(tpl);
        h.click("Fill out");
        h.click("Answer 1");
        h.type_text(answer);
        h.click("Submit");
        h.click("Back to all forms");
    }
    h.click("Open form IT Requests");
    h.click("Answers");
    h.click("Done answers");
    assert!(!h.has("Answer task No wifi"), "not done yet");
    h.click("Open form Feedback Form");
    h.click("Answers");
    assert_eq!(h.app.sweep.forms.answers_filter, 0);
    assert!(h.has("Answer task Loved it"));
}

/// A form opened from the search opens on Edit, as from the sidebar or the
/// table, not on whichever tab the last form was left on.
#[test]
fn a_form_from_the_search_opens_on_edit() {
    let mut h = Ui1::new();
    open_forms(&mut h);
    h.click("Feedback Form");
    h.click("Back to all forms");
    h.click("IT Requests");
    h.click("Answers");
    h.click("Search");
    h.type_text("Feedback");
    h.click("Feedback Form");
    assert_eq!(h.app.form_open, Some(form_named(&h, "Feedback Form")));
    assert_eq!(h.app.form_tab, FormTab::Edit);
}
