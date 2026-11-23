// Click-through tests for Edit profile: full name, display name, title and
// photo, and others seeing the display name before the full name.

use super::*;
use crate::app::tests::test_picture;

/// The name at the top of the profile menu opens it (there is no row for it).
fn open_edit_profile(h: &mut Ui1) {
    h.click("Profile menu");
    h.settle();
    h.click("Edit profile");
    h.settle();
}

#[test]
fn edit_profile_saves_the_names_title_and_a_square_photo() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    open_edit_profile(&mut h);
    for s in [
        "Edit profile",
        "Full name",
        "Display name",
        "Title",
        "Profile photo",
        "Upload photo",
    ] {
        assert!(h.has(s), "the dialog shows {}", s);
    }
    assert!(!h.has("Remove photo"), "nothing to remove yet");
    h.click("Profile display name");
    h.type_text("Jiwon");
    h.click("Profile title");
    h.type_text("Designer");
    crate::media::PICKED_PICTURE.with(|p| *p.borrow_mut() = Some(test_picture("me.png", 300, 200)));
    h.click("Upload photo");
    h.until("the photo is ready", |h| {
        h.app
            .profile_photo
            .as_ref()
            .is_some_and(|p| p.att.is_some())
    });
    h.settle();
    assert!(h.has("Remove photo"));
    assert!(
        h.app.data.member(me).unwrap().photo.is_none(),
        "nothing changes before Save"
    );
    h.click("Save changes");
    assert!(h.app.modal.is_none(), "the dialog closes");
    let m = h.app.data.member(me).unwrap().clone();
    assert_eq!(
        (m.display_name.as_str(), m.title.as_str()),
        ("Jiwon", "Designer")
    );
    assert_eq!(m.name, "Me", "the full name stays");
    assert_eq!(m.shown(), "Jiwon", "others see the display name first");
    // the photo is the picture's middle square, kept in the file cache
    let photo = m.photo.clone().expect("a photo");
    let path = h.app.files.local(&photo.file).expect("in the cache");
    let img = image::load_from_memory(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!((img.width(), img.height()), (200, 200));
    // and the avatars draw it
    h.until("the photo is decoded for avatars", |h| {
        crate::widgets::avatar_texture(&h.ctx, &photo.file).is_some()
    });
    h.settle();
    let tex = crate::widgets::avatar_texture(&h.ctx, &photo.file).unwrap();
    assert!(
        h.shapes
            .iter()
            .any(|c| matches!(&c.shape, egui::Shape::Mesh(mesh) if mesh.texture_id == tex)),
        "an avatar painted with the photo"
    );
    // the profile menu shows the display name and the title
    h.click("Profile menu");
    h.settle();
    assert!(h.has("Jiwon") && h.has("Designer"));
    h.key(egui::Key::Escape);
    // reopened, the dialog has what was saved; Remove photo then Save drops it
    open_edit_profile(&mut h);
    h.click("Remove photo");
    h.settle();
    h.click("Save changes");
    assert!(h.app.data.member(me).unwrap().photo.is_none());
}

#[test]
fn edit_profile_shows_the_photo_left_of_the_names() {
    let mut h = Ui1::new();
    open_edit_profile(&mut h);
    let photo = h.rect("Profile photo");
    let name = h.rect("Profile full name");
    assert!(
        photo.right() < name.left(),
        "the photo {:?} is left of the full name {:?}",
        photo,
        name
    );
    assert!(h.rect("Upload photo").right() < name.left());
}

#[test]
fn edit_profile_fills_in_the_email_i_sign_in_with() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let set_email = |h: &mut Ui1, e: &str| {
        h.app
            .data
            .members
            .iter_mut()
            .find(|m| m.id == me)
            .unwrap()
            .email = e.into();
    };
    set_email(&mut h, "");
    h.app.sync_rt.account = Some("me@team.io".into());
    open_edit_profile(&mut h);
    assert!(
        matches!(&h.app.modal, Some(Modal::Profile { email, .. }) if email == "me@team.io"),
        "an empty email starts as the sign-in email"
    );
    assert!(
        !h.painted_text("me@team.io").is_empty(),
        "shown in the field"
    );
    assert!(
        h.app.data.member(me).unwrap().email.is_empty(),
        "nothing changes before Save"
    );
    h.click("Save changes");
    assert_eq!(h.app.data.member(me).unwrap().email, "me@team.io");
    // an email of my own stays
    set_email(&mut h, "work@team.io");
    open_edit_profile(&mut h);
    assert!(matches!(&h.app.modal, Some(Modal::Profile { email, .. }) if email == "work@team.io"));
    h.click("Cancel");
    // Settings > My settings starts with it too
    set_email(&mut h, "");
    h.app.open_settings("My settings");
    h.settle();
    assert!(!h.painted_text("me@team.io").is_empty());
}

#[test]
fn a_photo_only_takes_jpg_png_or_webp() {
    let mut h = Ui1::new();
    open_edit_profile(&mut h);
    assert!(!h.painted_text("JPG, PNG or WebP only").is_empty());
    // (the name alone decides, so the file needn't really be a GIF)
    let png = test_picture("not-gif.png", 40, 40);
    let gif = png.with_file_name("me.gif");
    std::fs::copy(&png, &gif).unwrap();
    crate::media::PICKED_PICTURE.with(|p| *p.borrow_mut() = Some(gif));
    h.click("Upload photo");
    h.settle();
    assert!(h.app.profile_photo.is_none(), "a GIF isn't used");
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Choose a JPG, PNG or WebP picture.")
    );
    assert!(!h.has("Remove photo"));
    // a name ending in capitals is fine
    crate::media::PICKED_PICTURE.with(|p| *p.borrow_mut() = Some(test_picture("ME.PNG", 40, 40)));
    h.click("Upload photo");
    assert!(h.app.profile_photo.is_some());
}

#[test]
fn choosing_a_photo_moves_nothing_in_the_dialog() {
    use crate::i18n::{set_lang, Lang};
    for lang in [Lang::En, Lang::Ko] {
        let mut h = Ui1::new();
        set_lang(lang);
        // (the buttons are named by their words, in Korean too)
        let upload = crate::i18n::tr("Upload photo");
        let remove = crate::i18n::tr("Remove photo");
        h.app.open_profile();
        h.settle();
        let before = (
            h.modal_rect(),
            h.rect("Profile email"),
            h.rect("Avatar color 1"),
            h.rect(upload),
        );
        crate::media::PICKED_PICTURE
            .with(|p| *p.borrow_mut() = Some(test_picture("me.png", 60, 60)));
        h.click(upload);
        h.until("the photo is ready", |h| {
            h.app
                .profile_photo
                .as_ref()
                .is_some_and(|p| p.att.is_some())
        });
        h.settle();
        assert!(h.has(remove));
        let after = (
            h.modal_rect(),
            h.rect("Profile email"),
            h.rect("Avatar color 1"),
            h.rect(upload),
        );
        let help = h.painted_text(crate::i18n::tr("JPG, PNG or WebP only"));
        assert!(
            help.len() == 1 && help[0].0.height() < 16.0,
            "the formats fit one line under the button: {:?}",
            help
        );
        set_lang(Lang::En);
        assert_eq!(before, after, "Remove photo shows in room kept for it");
    }
}

#[test]
fn a_photo_chosen_then_cancelled_changes_nothing() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    open_edit_profile(&mut h);
    h.click("Profile display name");
    h.type_text("Nick");
    crate::media::PICKED_PICTURE
        .with(|p| *p.borrow_mut() = Some(test_picture("other.png", 80, 120)));
    h.click("Upload photo");
    h.until("the photo is ready", |h| {
        h.app
            .profile_photo
            .as_ref()
            .is_some_and(|p| p.att.is_some())
    });
    // Remove photo appears and moves the buttons below it
    h.settle();
    assert!(h.has("Remove photo"));
    h.click("Cancel");
    assert!(h.app.modal.is_none());
    let m = h.app.data.member(me).unwrap();
    assert!(m.photo.is_none() && m.display_name.is_empty());
    assert!(h.app.profile_photo.is_none(), "the chosen photo is dropped");
}

#[test]
fn others_are_shown_by_display_name_else_full_name() {
    let mut h = Ui1::new();
    let alex = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.name == "Alex Kim")
        .unwrap()
        .id;
    h.app
        .data
        .members
        .iter_mut()
        .find(|m| m.id == alex)
        .unwrap()
        .display_name = "Lexi".into();
    h.app
        .data
        .members
        .iter_mut()
        .find(|m| m.id == alex)
        .unwrap()
        .title = "Engineer".into();
    h.app.open_settings("People");
    h.settle();
    assert!(
        !h.painted_text("Lexi").is_empty(),
        "People shows the display name"
    );
    assert!(h.painted_text("Alex Kim").is_empty(), "not the full name");
    assert!(
        !h.painted_text("Engineer").is_empty(),
        "with the title under it"
    );
    assert!(
        !h.painted_text("Sam Lee").is_empty(),
        "and the full name of one without"
    );
    // the initials follow the name shown
    assert_eq!(h.app.data.member(alex).unwrap().initials(), "L");
    // either name mentions them
    let members = h.app.data.members.clone();
    assert_eq!(
        crate::model::mentioned_ids("@Lexi hi", &members, &[]),
        vec![alex]
    );
    assert_eq!(
        crate::model::mentioned_ids("@Alex Kim hi", &members, &[]),
        vec![alex]
    );
    // a display name cleared (spaces only) falls back to the full name
    h.app
        .data
        .members
        .iter_mut()
        .find(|m| m.id == alex)
        .unwrap()
        .display_name = "  ".into();
    assert_eq!(h.app.data.member(alex).unwrap().shown(), "Alex Kim");
}

#[test]
fn members_from_older_files_read_without_the_new_fields() {
    let m: Member =
        serde_json::from_str(r#"{"id": 7, "name": "Old Timer", "color": [1, 2, 3]}"#).unwrap();
    assert_eq!(m.shown(), "Old Timer");
    assert!(m.display_name.is_empty() && m.title.is_empty() && m.photo.is_none());
    // the new fields are always written, so clearing one reaches the others
    let v = serde_json::to_value(&m).unwrap();
    assert_eq!(v["display_name"], "");
    assert_eq!(v["title"], "");
}

#[test]
fn edit_profile_opens_at_its_width_without_growing() {
    let mut h = Ui1::new();
    open_edit_profile(&mut h);
    let first = h.modal_rect();
    for _ in 0..40 {
        h.frame(vec![]);
    }
    let later = h.modal_rect();
    assert_eq!(
        first.width(),
        later.width(),
        "the dialog keeps its width: {:?} then {:?}",
        first,
        later
    );
}

/// From someone's profile (their name on a message), the owner or an admin
/// changes the names everyone sees them by: someone whose phone app has no
/// Edit profile yet can't. A plain member on the team server gets no such
/// button, nor an admin for the owner (the server turns both down).
#[test]
fn an_owner_or_admin_changes_someone_elses_name() {
    let mut h = Ui1::new();
    h.app.section = Section::Posts;
    h.settle();
    h.hover(pos2(900.0, 600.0));
    h.click("general");
    let list = Rect::from_min_max(pos2(330.0, 120.0), pos2(1440.0, 760.0));
    let at = h.rect_in("Alex Kim", list).center();
    h.click_at(at);
    h.settle();
    assert!(h.has("Close profile"), "Alex's profile opens");
    let alex = h.app.chat.profile.unwrap();
    // alone on this computer anyone may
    h.click("Change name of Alex Kim");
    assert!(matches!(h.app.modal, Some(Modal::MemberName { .. })));
    assert!(h.has("Member full name") && h.has("Member display name"));
    h.click("Member display name");
    h.type_text("Lexi");
    h.click("Save changes");
    assert!(h.app.modal.is_none(), "the dialog closes");
    let m = h.app.data.member(alex).unwrap().clone();
    assert_eq!(
        (m.name.as_str(), m.display_name.as_str()),
        ("Alex Kim", "Lexi")
    );
    h.settle();
    assert!(
        h.has("Change name of Lexi"),
        "the profile shows the new name"
    );

    // Cancel changes nothing
    h.click("Change name of Lexi");
    h.click("Member display name");
    h.type_text("xx");
    h.click("Cancel");
    assert_eq!(h.app.data.member(alex).unwrap().display_name, "Lexi");

    // on the team server
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    for (mine, theirs, offered) in [
        (wb_net::Role::Member, MemberRole::Admin, false),
        (wb_net::Role::Admin, MemberRole::Admin, true),
        (wb_net::Role::Admin, MemberRole::Owner, false),
        (wb_net::Role::Owner, MemberRole::Admin, true),
    ] {
        h.app.sync_rt.role = Some(mine);
        h.app
            .data
            .members
            .iter_mut()
            .find(|m| m.id == alex)
            .unwrap()
            .role = theirs.clone();
        h.settle();
        assert!(h.has("Close profile"));
        assert_eq!(
            h.has("Change name of Lexi"),
            offered,
            "{mine:?} changing an {theirs:?}'s name"
        );
    }
    // my own profile has Edit profile instead
    let me = h.app.data.me;
    h.app.chat.profile = Some(me);
    h.settle();
    assert!(h.has("Edit profile") && !h.has("Change name"));
}
