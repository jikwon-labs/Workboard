// Click-through tests for pictures and attachments in chat and on tasks.

use super::*;

/// A picture file for attachment tests.
fn test_picture(name: &str, w: u32, h: u32) -> std::path::PathBuf {
    let dir = temp_home("ui-pic");
    std::fs::create_dir_all(&dir).unwrap();
    let mut img = image::RgbaImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([(x * 255 / w) as u8, (y * 255 / h) as u8, 160, 255]);
    }
    let p = dir.join(name);
    img.save(&p).unwrap();
    p
}

#[test]
fn a_picture_can_be_sent_while_the_server_is_down_and_goes_up_later() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-ui-offline-pic".into(),
        tag: 1,
        server: Some("127.0.0.1:9".into()),
    });
    crate::app::together::save_token("127.0.0.1:9", "dev-ui-offline-pic", "tok");
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.dropped = vec![egui::DroppedFile {
        path: Some(test_picture("offline.png", 60, 40)),
        ..Default::default()
    }];
    h.frame(vec![]);
    // nothing listens on port 9: the upload fails, but the message may go
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !matches!(
        h.app
            .chat
            .files
            .first()
            .and_then(|p| p.transfer(&h.app.files)),
        Some(crate::app::files::Transfer::Failed(_))
    ) && std::time::Instant::now() < until
    {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let Some(crate::app::files::Transfer::Failed(why)) = h.app.chat.files[0].transfer(&h.app.files)
    else {
        panic!("the upload failed");
    };
    // said without the system's English words
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    let shown = App::error_text(&why);
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert!(
        !shown.contains("os error") && !shown.contains("refused"),
        "{}",
        shown
    );
    let (ok, files) = h.app.pending_ready(false);
    assert!(ok && files.len() == 1, "the message can go");
    h.click("Message composer");
    h.type_text("from the train");
    h.key(egui::Key::Enter);
    let m = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .clone();
    crate::app::together::forget_token("127.0.0.1:9", "dev-ui-offline-pic");
    assert_eq!(m.text, "from the train");
    assert_eq!(m.attachments.len(), 1);
    assert!(
        h.app.files.is_waiting(&m.attachments[0].file),
        "the picture waits to go up, also after a restart"
    );
}

#[test]
fn drop_a_picture_send_it_and_open_it_large() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let n = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    // dragging over the window says where to drop
    let pic = test_picture("beach.png", 600, 400);
    h.frame(vec![]);
    // dropped from Finder: it waits in the message box until it's ready
    h.dropped = vec![egui::DroppedFile {
        path: Some(pic.clone()),
        ..Default::default()
    }];
    h.frame(vec![]);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !(h.app.pending_ready(false).0 && !h.app.pending_ready(false).1.is_empty())
        && std::time::Instant::now() < until
    {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.settle();
    assert!(h.has("Attachment beach.png"), "a tile in the message box");
    // a picture alone can be sent: Enter in the empty box
    h.click("Message composer");
    h.key(egui::Key::Enter);
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert_eq!(ch.messages.len(), n + 1, "sent");
    let m = ch.messages.last().unwrap().clone();
    assert_eq!(m.text, "");
    assert_eq!(m.attachments.len(), 1);
    assert_eq!(
        (
            m.attachments[0].w,
            m.attachments[0].h,
            m.attachments[0].kind
        ),
        (600, 400, MediaKind::Image)
    );
    assert!(h.app.chat.files.is_empty(), "the box is empty again");
    assert!(!h.has("Attachment beach.png"));
    // it shows in the conversation; clicking opens it large, Esc closes
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h
        .app
        .files
        .texture(&m.attachments[0].file, crate::app::files::THUMB)
        .is_none()
        && std::time::Instant::now() < until
    {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        h.app
            .files
            .texture(&m.attachments[0].file, crate::app::files::THUMB)
            .is_some(),
        "the picture was decoded for the screen"
    );
    h.click("Picture beach.png");
    assert!(h.app.viewer.is_some());
    let reveal = if cfg!(windows) {
        "Show in File Explorer"
    } else {
        "Show in Finder"
    };
    assert!(h.has("Large beach.png") && h.has(reveal) && h.has("Save to Downloads"));
    h.key(egui::Key::Escape);
    assert!(h.app.viewer.is_none());
    // a file that isn't a picture or video goes too, shown by its name
    let md = pic.with_file_name("ui-notes.md");
    std::fs::write(&md, "# Plan\n- ship it\n").unwrap();
    h.dropped = vec![egui::DroppedFile {
        path: Some(md),
        ..Default::default()
    }];
    h.frame(vec![]);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !(h.app.pending_ready(false).0 && !h.app.pending_ready(false).1.is_empty())
        && std::time::Instant::now() < until
    {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(h.app.chat.files[0].error, None);
    h.settle();
    assert!(h.has("Attachment ui-notes.md"), "a tile in the message box");
    h.click("Message composer");
    h.type_text("the plan");
    h.key(egui::Key::Enter);
    let m = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .clone();
    assert_eq!(m.text, "the plan");
    assert_eq!(
        (m.attachments[0].kind, m.attachments[0].bytes),
        (MediaKind::File, 17)
    );
    h.settle();
    // clicking it saves it to Downloads instead of opening it large
    let saved = std::env::temp_dir()
        .join("workboard-test-downloads")
        .join("ui-notes.md");
    let _ = std::fs::remove_file(&saved);
    h.click("File ui-notes.md");
    h.app.apply_actions();
    h.settle();
    assert!(h.app.viewer.is_none(), "not opened large");
    assert_eq!(
        std::fs::read_to_string(&saved).unwrap(),
        "# Plan\n- ship it\n"
    );
    assert!(h
        .app
        .toast
        .as_ref()
        .is_some_and(|(t, _)| t.contains("ui-notes.md")));
    let _ = std::fs::remove_file(&saved);
}

#[test]
fn pictures_dropped_on_a_task_attach_and_can_be_removed() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    let pic = test_picture("mockup.png", 300, 200);
    h.dropped = vec![egui::DroppedFile {
        path: Some(pic),
        ..Default::default()
    }];
    h.frame(vec![]);
    assert!(h.app.chat.files.is_empty(), "not taken by the chat box");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h.app.task(id).unwrap().attachments.is_empty() && std::time::Instant::now() < until {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let t = h.app.task(id).unwrap();
    assert_eq!(t.attachments.len(), 1);
    assert_eq!(
        (t.attachments[0].name.as_str(), t.attachments[0].w),
        ("mockup.png", 300)
    );
    assert!(h.app.task_files.is_empty());
    h.settle();
    assert!(h.has("Attachment mockup.png"));
    h.click("Remove attachment mockup.png");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().attachments.is_empty());
}

#[test]
fn a_pdf_dropped_on_a_task_attaches_and_a_click_saves_it() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    let dir = temp_home("ui-task-pdf");
    std::fs::create_dir_all(&dir).unwrap();
    let pdf = dir.join("ui-brief.pdf");
    std::fs::write(&pdf, b"%PDF-1.4 brief").unwrap();
    h.dropped = vec![egui::DroppedFile {
        path: Some(pdf),
        ..Default::default()
    }];
    h.frame(vec![]);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h.app.task(id).unwrap().attachments.is_empty() && std::time::Instant::now() < until {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let a = h.app.task(id).unwrap().attachments[0].clone();
    assert_eq!((a.name.as_str(), a.kind), ("ui-brief.pdf", MediaKind::File));
    h.settle();
    let saved = std::env::temp_dir()
        .join("workboard-test-downloads")
        .join("ui-brief.pdf");
    let _ = std::fs::remove_file(&saved);
    h.click("Attachment ui-brief.pdf");
    h.settle();
    assert_eq!(std::fs::read(&saved).unwrap(), b"%PDF-1.4 brief");
    let _ = std::fs::remove_file(&saved);
}

#[test]
fn a_task_attachment_that_cannot_go_up_says_why_and_can_be_tried_again() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-ui-task-fail".into(),
        tag: 1,
        server: Some("127.0.0.1:9".into()),
    });
    crate::app::together::save_token("127.0.0.1:9", "dev-ui-task-fail", "tok");
    let id = open_design_task(&mut h);
    h.dropped = vec![egui::DroppedFile {
        path: Some(test_picture("refused.png", 40, 30)),
        ..Default::default()
    }];
    h.frame(vec![]);
    // the server can't be reached: not "Adding..." for ever, but why, and
    // a way to try again
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !h.has("Try again refused.png") && std::time::Instant::now() < until {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(h.has("Try again refused.png"));
    assert!(h.app.task(id).unwrap().attachments.is_empty());
    let sha = h.app.task_files[0].1.att.clone().unwrap().file;
    assert!(h.app.files.is_waiting(&sha), "also tried again by itself");
    h.click("Try again refused.png");
    assert_eq!(h.app.task_files.len(), 1);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !h.has("Dismiss refused.png") && std::time::Instant::now() < until {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.click("Dismiss refused.png");
    crate::app::together::forget_token("127.0.0.1:9", "dev-ui-task-fail");
    assert!(h.app.task_files.is_empty());
    assert!(
        !h.app.files.is_waiting(&sha),
        "and no longer waits to go up"
    );
}

#[test]
fn a_video_the_app_cannot_play_offers_another_app() {
    // every video outside macOS, and one AVFoundation gives up on
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let src = temp_home("ui-video").with_extension("mp4");
    std::fs::write(&src, b"not really a video").unwrap();
    let (sha, bytes) = wb_net::sha256_file(&src).unwrap();
    std::fs::create_dir_all(h.app.files.dir()).unwrap();
    std::fs::copy(&src, h.app.files.dir().join(&sha)).unwrap();
    let mut m =
        crate::app::slack::new_message(h.app.data.new_id(), h.app.data.me, String::new(), now_ts());
    m.attachments.push(Attachment {
        file: sha,
        name: "clip.mp4".into(),
        kind: MediaKind::Video,
        bytes,
        w: 0,
        h: 0,
        secs: 0.0,
        thumb: None,
    });
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .push(m);
    h.settle();
    h.click("Video clip.mp4");
    assert!(h.app.viewer.is_some());
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !h.has("Open in another app") && std::time::Instant::now() < until {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        h.has("Open in another app"),
        "the system's player is offered"
    );
    h.click("Open in another app");
    assert!(h.app.viewer.is_some(), "the viewer stays open");
    h.key(egui::Key::Escape);
    assert!(h.app.viewer.is_none());
}

/// The V of a Cmd+V as egui passes it on: its press is kept back, so only
/// the release shows, with Cmd on it while Cmd is still held.
fn v_released(cmd: bool) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::V,
        physical_key: Some(egui::Key::V),
        pressed: false,
        repeat: false,
        modifiers: if cmd {
            egui::Modifiers::COMMAND
        } else {
            egui::Modifiers::NONE
        },
    }
}

fn wait_for_tiles(h: &mut Ui1, n: usize) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !(h.app.chat.files.len() >= n && h.app.pending_ready(false).0)
        && std::time::Instant::now() < until
    {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.settle();
}

#[test]
fn a_screenshot_pastes_into_the_message_box_without_clicking_it_first() {
    let mut h = Ui1::new();
    h.click("general");
    let shot = std::fs::read(test_picture("shot.png", 40, 30)).unwrap();
    crate::media::CLIPBOARD.with(|c| c.borrow_mut().png = Some(shot));
    // back from taking the screenshot in another app: a click on the
    // messages, so nothing has the keyboard
    let messages = h.find("Message composer").unwrap() - vec2(0.0, 300.0);
    h.click_at(messages);
    assert_eq!(h.ctx.memory(|m| m.focused()), None);
    h.frame(vec![v_released(true)]);
    wait_for_tiles(&mut h, 1);
    assert!(h.has("Attachment Pasted image.png"), "a tile in the box");
    assert_eq!(
        h.ctx.memory(|m| m.focused()),
        Some(egui::Id::new(super::super::files::CHAT_BOX)),
        "the cursor waits in the box, so Enter sends it"
    );
    h.key(egui::Key::Enter);
    let cid = h.app.chat.open.unwrap();
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert_eq!(ch.messages.last().unwrap().attachments.len(), 1, "sent");

    // Cmd let go before V: the release has no Cmd on it but is still a paste
    h.click_at(messages);
    h.frame_held(vec![], egui::Modifiers::COMMAND);
    h.frame(vec![v_released(false)]);
    wait_for_tiles(&mut h, 1);
    assert_eq!(h.app.chat.files.len(), 1, "pasted again");

    // typing a v is not a paste, even right after a Cmd shortcut
    h.click("Message composer");
    h.frame_held(vec![], egui::Modifiers::COMMAND);
    h.frame(vec![
        egui::Event::Key {
            key: egui::Key::V,
            physical_key: Some(egui::Key::V),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Text("v".into()),
    ]);
    h.frame(vec![v_released(false)]);
    h.settle();
    assert_eq!(h.app.chat.files.len(), 1, "no second picture");

    // pasting while another text field has the keyboard leaves the message box alone
    h.click("Search");
    h.settle();
    let typing = h.ctx.memory(|m| m.focused());
    assert!(typing.is_some_and(|id| egui::TextEdit::load_state(&h.ctx, id).is_some()));
    h.frame(vec![v_released(true)]);
    h.settle();
    assert_eq!(
        h.app.chat.files.len(),
        1,
        "nothing added from the search box"
    );
}

/// The filled rectangles painted inside `area`, in paint order.
fn fills_inside(h: &Ui1, area: Rect) -> Vec<(Rect, egui::Color32)> {
    h.shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Rect(r) if area.expand(0.5).contains_rect(r.rect) => {
                Some((r.rect, r.fill))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn pictures_look_the_same_under_the_pointer() {
    // a picture in a chat message: nothing is laid over it on hover
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let mut m =
        crate::app::slack::new_message(h.app.data.new_id(), h.app.data.me, String::new(), now_ts());
    m.attachments.push(Attachment {
        file: "no-such-file".into(),
        name: "sky.png".into(),
        kind: MediaKind::Image,
        bytes: 10,
        w: 400,
        h: 300,
        secs: 0.0,
        thumb: None,
    });
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .push(m);
    h.settle();
    let tile = h.rect("Picture sky.png");
    // beside the picture on the same message, so the row looks hovered both times
    h.hover(pos2(tile.right() + 40.0, tile.center().y));
    let off = fills_inside(&h, tile);
    h.hover(tile.center());
    assert_eq!(h.cursor, egui::CursorIcon::PointingHand, "still clickable");
    assert_eq!(fills_inside(&h, tile), off, "no hover layer on the picture");

    // a picture attached to a task: no hover fill either
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
    t.attachments.push(Attachment {
        file: "no-such-file".into(),
        name: "plan.png".into(),
        kind: MediaKind::Image,
        bytes: 10,
        w: 400,
        h: 300,
        secs: 0.0,
        thumb: None,
    });
    h.settle();
    h.scroll_into_view("Attachment plan.png");
    let tile = h.rect("Attachment plan.png");
    let pic = pos2(tile.center().x, tile.top() + 45.0);
    h.hover(pic);
    assert!(!h.hover_fill_at(pic), "no hover fill on the picture");
}
