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
