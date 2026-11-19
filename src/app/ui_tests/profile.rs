// Click-through tests for Edit profile: full name, display name, title and
// photo, and others seeing the display name before the full name.

use super::*;

/// A picture file wider than it is tall.
fn test_picture(name: &str, w: u32, h: u32) -> std::path::PathBuf {
    let dir = temp_home("ui-profile-pic");
    std::fs::create_dir_all(&dir).unwrap();
    let mut img = image::RgbaImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([(x * 255 / w) as u8, (y * 255 / h) as u8, 160, 255]);
    }
    let p = dir.join(name);
    img.save(&p).unwrap();
    p
}

/// Frames until `done` holds (or fail after 10 seconds).
fn frames_until(h: &mut Ui1, what: &str, done: impl Fn(&Ui1) -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !done(h) {
        assert!(std::time::Instant::now() < until, "never: {}", what);
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

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
    frames_until(&mut h, "the photo is ready", |h| {
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
    frames_until(&mut h, "the photo is decoded for avatars", |h| {
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
fn a_photo_chosen_then_cancelled_changes_nothing() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    open_edit_profile(&mut h);
    h.click("Profile display name");
    h.type_text("Nick");
    crate::media::PICKED_PICTURE
        .with(|p| *p.borrow_mut() = Some(test_picture("other.png", 80, 120)));
    h.click("Upload photo");
    frames_until(&mut h, "the photo is ready", |h| {
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
