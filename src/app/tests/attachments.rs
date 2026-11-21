// Behaviour tests for pictures, videos and task attachments: uploads and
// downloads through a server or a shared folder, retries, and the local
// file cache.

use super::*;

fn picture_file(name: &str, w: u32, h: u32) -> std::path::PathBuf {
    let dir = temp_home("pic");
    std::fs::create_dir_all(&dir).unwrap();
    let mut img = image::RgbaImage::new(w, h);
    for (x, _, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([if x < w / 2 { 200 } else { 30 }, 90, 40, 255]);
    }
    let p = dir.join(name);
    img.save(&p).unwrap();
    p
}

/// Tick until the message box's attachments are ready (or fail the test).
fn until_ready(a: &mut App, thread: bool) -> Vec<Attachment> {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while std::time::Instant::now() < end {
        a.files_tick(None);
        a.net_tick(None);
        let (ok, files) = a.pending_ready(thread);
        let list = if thread {
            &a.chat.thread_files
        } else {
            &a.chat.files
        };
        if let Some(e) = list.iter().find_map(|p| p.error.clone()) {
            panic!("attachment refused: {}", e);
        }
        // sendable already while going up; these tests want it there
        let up = list.iter().any(|p| {
            p.att.as_ref().is_some_and(|x| {
                matches!(
                    a.files.state(&x.file),
                    Some(crate::app::files::Transfer::Up(_))
                )
            })
        });
        if ok && !up && !files.is_empty() && files.len() == list.len() {
            return files;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("attachments never got ready: {:?}", a.chat.files);
}

#[test]
fn a_picture_goes_through_the_server_to_the_other_computer() {
    let server = TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);
    let mut a = app();
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "A shared on the server", |a| {
        a.data.extra.shared.is_some()
    });
    net_until(&mut a, "A sees the people list", |a| {
        a.sync_rt.people.iter().any(|p| p.member == 3)
    });
    a.access_request(wb_net::Msg::CreateInvite {
        member: 3,
        role: wb_net::Role::Member,
    });
    net_until(&mut a, "invite code", |a| {
        a.sync_rt
            .people
            .iter()
            .any(|p| p.member == 3 && p.invite.is_some())
    });
    let code = a
        .sync_rt
        .people
        .iter()
        .find(|p| p.member == 3)
        .unwrap()
        .invite
        .clone()
        .unwrap();
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.connect_server(&addr, &code).unwrap();
    net_until(&mut b, "B joined", |b| b.data.extra.shared.is_some());

    // A puts a picture in the message box: it's checked, then sent to the server
    let ch = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    let pic = picture_file("sunset.png", 320, 200);
    a.add_media(false, Some(pic.clone()), None);
    let files = until_ready(&mut a, false);
    assert_eq!(files.len(), 1);
    let att = files[0].clone();
    assert_eq!(
        (att.name.as_str(), att.kind, att.w, att.h),
        ("sunset.png", MediaKind::Image, 320, 200)
    );
    assert_eq!(att.bytes, std::fs::metadata(&pic).unwrap().len());
    assert_eq!(
        std::fs::read(
            server
                .home
                .join(workboard_server::FILES_DIR)
                .join(&att.file)
        )
        .unwrap(),
        std::fs::read(&pic).unwrap(),
        "on the server under its hash"
    );
    a.apply_chat(vec![ChatAction::Send {
        ch,
        text: "look at this".into(),
        parent: None,
        files,
    }]);
    a.sync_note_change();
    a.write_shared();

    // B gets the message, and the picture when it comes on screen
    net_until(&mut b, "A's picture message at B", |b| {
        b.data.channels.iter().any(|c| {
            c.messages
                .iter()
                .any(|m| m.text == "look at this" && m.attachments.len() == 1)
        })
    });
    let m = b
        .data
        .channels
        .iter()
        .flat_map(|c| c.messages.iter())
        .find(|m| m.text == "look at this")
        .unwrap()
        .clone();
    assert_eq!(m.attachments[0], att, "the same attachment details");
    assert!(
        b.files.local(&att.file).is_none(),
        "not fetched before it's shown"
    );
    let remote = b.media_remote();
    assert!(matches!(remote, crate::app::files::Remote::Server { .. }));
    b.files.want(&att, crate::app::files::THUMB);
    let sha = att.file.clone();
    // the screen asks again every frame while the message is visible
    b.files.settle(&remote, |f| {
        f.want(&att, crate::app::files::THUMB);
        f.local(&sha).is_some() && f.decoded(&sha, crate::app::files::THUMB)
    });
    assert_eq!(
        std::fs::read(b.files.local(&att.file).unwrap()).unwrap(),
        std::fs::read(&pic).unwrap()
    );
}

#[test]
fn a_pdf_goes_through_the_server_and_the_other_computer_saves_it() {
    let server = TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);
    let mut a = app();
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "A shared on the server", |a| {
        a.data.extra.shared.is_some()
    });
    net_until(&mut a, "A sees the people list", |a| {
        a.sync_rt.people.iter().any(|p| p.member == 3)
    });
    a.access_request(wb_net::Msg::CreateInvite {
        member: 3,
        role: wb_net::Role::Member,
    });
    net_until(&mut a, "invite code", |a| {
        a.sync_rt
            .people
            .iter()
            .any(|p| p.member == 3 && p.invite.is_some())
    });
    let code = a
        .sync_rt
        .people
        .iter()
        .find(|p| p.member == 3)
        .unwrap()
        .invite
        .clone()
        .unwrap();
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.connect_server(&addr, &code).unwrap();
    net_until(&mut b, "B joined", |b| b.data.extra.shared.is_some());

    let ch = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    let dir = temp_home("pdf");
    std::fs::create_dir_all(&dir).unwrap();
    let pdf = dir.join("Quarterly plan.pdf");
    let body: Vec<u8> = b"%PDF-1.4\n"
        .iter()
        .copied()
        .chain((0..5000u32).map(|i| (i % 251) as u8))
        .collect();
    std::fs::write(&pdf, &body).unwrap();
    a.add_media(false, Some(pdf), None);
    let files = until_ready(&mut a, false);
    let att = files[0].clone();
    assert_eq!((att.kind, att.bytes), (MediaKind::File, body.len() as u64));
    assert_eq!(
        std::fs::read(
            server
                .home
                .join(workboard_server::FILES_DIR)
                .join(&att.file)
        )
        .unwrap(),
        body,
        "on the server under its hash"
    );
    a.apply_chat(vec![ChatAction::Send {
        ch,
        text: String::new(),
        parent: None,
        files,
    }]);
    a.sync_note_change();
    a.write_shared();

    // B can read the team's data with the new kind in it
    net_until(&mut b, "A's file message at B", |b| {
        b.data.channels.iter().any(|c| {
            c.messages
                .iter()
                .any(|m| m.attachments.iter().any(|x| x.kind == MediaKind::File))
        })
    });
    // showing it fetches nothing: only a click does
    let remote = b.media_remote();
    b.files.want(&att, crate::app::files::THUMB);
    b.files.tick(None, &remote);
    assert_eq!(b.files.state(&att.file), None);
    let saved = std::env::temp_dir()
        .join("workboard-test-downloads")
        .join("Quarterly plan.pdf");
    let _ = std::fs::remove_file(&saved);
    b.save_file(&att);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !saved.is_file() && std::time::Instant::now() < end {
        b.files_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(std::fs::read(&saved).unwrap(), body, "saved to Downloads");
    assert!(b.file_saves.is_empty(), "nothing left waiting");
    let _ = std::fs::remove_file(&saved);
}

#[test]
fn attachments_wait_with_their_channel_and_only_media_goes() {
    let mut a = app();
    let chans: Vec<u64> = a.data.channels.iter().map(|c| c.id).take(2).collect();
    a.section = Section::Chat;
    a.chat.open = Some(chans[0]);
    a.drafts.chat_files.swap(a.chat.open, &mut a.chat.files);
    // no server, no folder: the picture is ready once it's in the cache
    assert_eq!(a.media_remote(), crate::app::files::Remote::None);
    a.add_media(false, Some(picture_file("one.png", 50, 40)), None);
    let files = until_ready(&mut a, false);
    assert!(a.files.local(&files[0].file).is_some());
    // another channel has its own (empty) box; coming back brings the picture back
    a.chat.open = Some(chans[1]);
    a.drafts.chat_files.swap(a.chat.open, &mut a.chat.files);
    assert!(a.chat.files.is_empty());
    a.chat.open = Some(chans[0]);
    a.drafts.chat_files.swap(a.chat.open, &mut a.chat.files);
    assert_eq!(a.chat.files.len(), 1);
    assert_eq!(
        a.chat.files[0].att.as_ref().map(|x| x.file.clone()),
        Some(files[0].file.clone())
    );
    // a pasted screenshot arrives as PNG bytes
    let png = std::fs::read(picture_file("shot.png", 30, 20)).unwrap();
    a.add_media(false, None, Some(png));
    let files = until_ready(&mut a, false);
    assert_eq!(files.len(), 2);
    assert_eq!(
        (files[1].name.as_str(), files[1].w, files[1].h),
        ("Pasted image.png", 30, 20)
    );
    // a thread reply can carry a picture too
    a.apply_chat(vec![ChatAction::Send {
        ch: chans[0],
        text: String::new(),
        parent: None,
        files: files.clone(),
    }]);
    let mid = a
        .data
        .channels
        .iter()
        .find(|c| c.id == chans[0])
        .unwrap()
        .messages
        .last()
        .unwrap()
        .id;
    a.apply_chat(vec![ChatAction::Send {
        ch: chans[0],
        text: "reply".into(),
        parent: Some(mid),
        files: files[..1].to_vec(),
    }]);
    let m = a
        .data
        .channels
        .iter()
        .find(|c| c.id == chans[0])
        .unwrap()
        .messages
        .last()
        .unwrap()
        .clone();
    assert_eq!(m.attachments.len(), 2);
    assert_eq!(m.replies[0].attachments.len(), 1);
    // messages without attachments don't grow a field in data.json
    let json = serde_json::to_value(&a.data).unwrap();
    let plain = json["channels"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["messages"].as_array().unwrap().iter())
        .find(|m| m["attachments"].is_null());
    assert!(plain.is_some());
    // any other file goes as it is, without a picture size
    let doc = temp_home("doc").join("plan.pdf");
    std::fs::create_dir_all(doc.parent().unwrap()).unwrap();
    std::fs::write(&doc, b"%PDF").unwrap();
    a.chat.files.clear();
    a.add_media(false, Some(doc), None);
    let files = until_ready(&mut a, false);
    assert_eq!(
        (
            files[0].name.as_str(),
            files[0].kind,
            files[0].bytes,
            files[0].w,
            files[0].h
        ),
        ("plan.pdf", MediaKind::File, 4, 0, 0)
    );
    assert_eq!(files[0].thumb, None);
    // a folder is refused, saying why
    let folder = temp_home("doc").join("Notes");
    std::fs::create_dir_all(&folder).unwrap();
    a.chat.files.clear();
    a.add_media(false, Some(folder), None);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while a.chat.files[0].error.is_none() && std::time::Instant::now() < end {
        a.files_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(a.chat.files[0].error.as_deref(), Some("Notes is a folder."));
}

#[test]
fn in_a_shared_folder_pictures_go_into_its_files_folder() {
    let folder = temp_home("shared-files");
    let mut a = app();
    a.share_here(&folder.to_string_lossy()).unwrap();
    assert_eq!(
        a.media_remote(),
        crate::app::files::Remote::Folder(folder.clone())
    );
    a.add_media(false, Some(picture_file("team.png", 40, 40)), None);
    let files = until_ready(&mut a, false);
    assert!(
        folder.join("files").join(&files[0].file).is_file(),
        "copied into the shared folder"
    );
    // the folder sync ignores it (only *.workboard.json files are read)
    assert!(crate::sync::other_files(&folder, "nobody")
        .iter()
        .all(|(p, _)| p.extension().is_some_and(|e| e == "json")));
    // another computer copies it from there into its own cache
    let b = App::build(None, Some(Data::seed()));
    let mut bf = crate::app::files::Files::new(temp_home("b-cache"));
    let remote = crate::app::files::Remote::Folder(folder.clone());
    bf.want(&files[0], crate::app::files::THUMB);
    let sha = files[0].file.clone();
    bf.settle(&remote, |f| f.local(&sha).is_some());
    drop(b);
}

#[test]
fn a_video_shows_a_still_from_the_cache() {
    let Some(ffmpeg) = ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
    else {
        eprintln!("skipped: ffmpeg isn't installed");
        return;
    };
    let dir = temp_home("video");
    std::fs::create_dir_all(&dir).unwrap();
    let clip = dir.join("Holiday.MP4");
    let ok = std::process::Command::new(ffmpeg)
        .args([
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=2:size=640x360:rate=30",
            "-pix_fmt",
            "yuv420p",
            "-c:v",
            "libx264",
        ])
        .arg(&clip)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok);
    let mut a = app();
    a.add_media(false, Some(clip), None);
    let files = until_ready(&mut a, false);
    let v = files[0].clone();
    assert_eq!((v.kind, v.w, v.h), (MediaKind::Video, 640, 360));
    assert!((v.secs - 2.0).abs() < 0.1, "{}", v.secs);
    // the cache names files by hash alone; the still must come out anyway
    let remote = a.media_remote();
    let sha = v.file.clone();
    a.files.settle(&remote, |f| {
        f.want(&v, crate::app::files::THUMB);
        f.decoded(&sha, crate::app::files::THUMB) || f.broken(&sha, crate::app::files::THUMB)
    });
    assert!(
        a.files.decoded(&sha, crate::app::files::THUMB),
        "a still from the video"
    );
    assert!(a
        .files
        .playable(&v)
        .unwrap()
        .to_string_lossy()
        .ends_with(".mp4"));
}

fn holds(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn a_picture_goes_without_where_it_was_taken() {
    // a JPEG whose EXIF names the place and the phone
    let mut img = image::RgbImage::new(48, 32);
    for (x, _, p) in img.enumerate_pixels_mut() {
        *p = image::Rgb([x as u8 * 5, 90, 40]);
    }
    let mut plain = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut plain)
        .encode_image(&img)
        .unwrap();
    let exif = b"Exif\0\0MM\0\x2A\0\0\0\x08\0\0\0\0\0\0 GPS +37.5665+126.9780 SecretPhone";
    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
    jpeg.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    jpeg.extend_from_slice(exif);
    jpeg.extend_from_slice(&plain[2..]);
    let dir = temp_home("gps");
    std::fs::create_dir_all(&dir).unwrap();
    let pic = dir.join("Lunch.jpg");
    std::fs::write(&pic, &jpeg).unwrap();

    let mut a = app();
    a.add_media(false, Some(pic.clone()), None);
    let files = until_ready(&mut a, false);
    let att = &files[0];
    assert_eq!((att.kind, att.w, att.h), (MediaKind::Image, 48, 32));
    let path = a.files.local(&att.file).unwrap();
    let kept = std::fs::read(&path).unwrap();
    assert!(!holds(&kept, b"+37.5665") && !holds(&kept, b"SecretPhone"));
    let (sha, bytes) = wb_net::sha256_file(&path).unwrap();
    assert_eq!(
        (att.file.clone(), att.bytes),
        (sha, bytes),
        "named by what is sent"
    );
    // the person's own file is left alone
    assert_eq!(std::fs::read(&pic).unwrap(), jpeg);
    // and no cleaned copy is left lying in the cache
    let left: Vec<_> = std::fs::read_dir(a.files.dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(".scrub-"))
        .collect();
    assert!(left.is_empty());
}

#[test]
fn a_video_goes_without_where_it_was_taken() {
    let Some(ffmpeg) = ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
    else {
        eprintln!("skipped: ffmpeg isn't installed");
        return;
    };
    let dir = temp_home("gpsvideo");
    std::fs::create_dir_all(&dir).unwrap();
    let clip = dir.join("Walk.mov");
    let ok = std::process::Command::new(ffmpeg)
        .args([
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=320x240:rate=30",
            "-pix_fmt",
            "yuv420p",
            "-c:v",
            "libx264",
            "-metadata",
            "location=+37.5665+126.9780/",
            "-metadata",
            "make=SecretPhone",
            "-movflags",
            "use_metadata_tags",
        ])
        .arg(&clip)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok);
    assert!(holds(&std::fs::read(&clip).unwrap(), b"+37.5665"));
    let mut a = app();
    a.add_media(false, Some(clip), None);
    let files = until_ready(&mut a, false);
    let v = files[0].clone();
    assert_eq!((v.kind, v.w, v.h), (MediaKind::Video, 320, 240));
    assert!((v.secs - 1.0).abs() < 0.1, "{}", v.secs);
    let kept = std::fs::read(a.files.local(&v.file).unwrap()).unwrap();
    assert!(!holds(&kept, b"+37.5665") && !holds(&kept, b"SecretPhone"));
    assert!(v.thumb.is_some(), "the cleaned video still gives a still");
}

#[test]
fn one_message_takes_at_most_ten_pictures() {
    let mut a = app();
    let pic = picture_file("many.png", 20, 20);
    // twelve chosen at once: ten go in, the rest are left out with a note
    let added: Vec<bool> = (0..12)
        .map(|_| a.add_media(false, Some(pic.clone()), None))
        .collect();
    assert_eq!(added.iter().filter(|x| **x).count(), 10);
    assert_eq!(a.chat.files.len(), crate::app::files::MAX_PER_MESSAGE);
    assert!(a
        .toast
        .as_ref()
        .is_some_and(|(t, _)| t.contains("up to 10")));
    // the thread's box counts on its own
    assert!(a.add_media(true, Some(pic.clone()), None));
    // removing one makes room again
    a.chat.files.pop();
    assert!(a.add_media(false, Some(pic), None));
    assert_eq!(a.chat.files.len(), 10);
}

#[test]
fn a_missing_file_is_fetched_again_later() {
    let folder = temp_home("retry-files");
    std::fs::create_dir_all(folder.join("files")).unwrap();
    let pic = picture_file("later.png", 30, 20);
    let (sha, bytes) = wb_net::sha256_file(&pic).unwrap();
    let att = Attachment {
        file: sha.clone(),
        name: "later.png".into(),
        kind: MediaKind::Image,
        bytes,
        w: 30,
        h: 20,
        secs: 0.0,
        thumb: None,
    };
    let mut f = crate::app::files::Files::new(temp_home("retry-cache"));
    let remote = crate::app::files::Remote::Folder(folder.clone());
    // not in the shared folder yet: the first try fails and the next one waits
    f.settle(&remote, |f| {
        f.want(&att, crate::app::files::THUMB);
        matches!(f.state(&sha), Some(crate::app::files::Transfer::Failed(_)))
            && f.fetch_tries(&sha) == Some((1, true))
    });
    // the sender may still be putting it there (a message goes before its
    // files): it's looked for again within seconds, and shown as loading
    assert!(f.on_its_way(&sha));
    std::fs::copy(&pic, folder.join("files").join(&sha)).unwrap();
    let start = std::time::Instant::now();
    f.settle(&remote, |f| {
        f.want(&att, crate::app::files::THUMB);
        f.local(&sha).is_some()
    });
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn without_a_sign_in_attachments_wait_and_the_token_is_kept() {
    let mut a = app();
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 1,
        server: Some("127.0.0.1:9".into()),
    });
    assert_eq!(
        a.media_remote(),
        crate::app::files::Remote::SignedOut("127.0.0.1:9".into())
    );
    a.add_media(false, Some(picture_file("wait.png", 20, 20)), None);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while a.chat.files[0].att.is_none() && std::time::Instant::now() < end {
        a.files_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let att = a.chat.files[0].att.clone().unwrap();
    assert_eq!(
        a.files.state(&att.file),
        Some(crate::app::files::Transfer::Failed(
            crate::app::files::NOT_SIGNED_IN.into()
        )),
        "not silently done"
    );
    // the message may go: the picture waits in the backlog and follows
    assert!(a.pending_ready(false).0);
    assert_eq!(a.pending_ready(false).1, vec![att.clone()]);
    // a sign-in is seen at once (the cached token is dropped when one is saved)
    crate::app::together::save_token("127.0.0.1:9", "dev1", "tok");
    assert!(
        matches!(a.media_remote(), crate::app::files::Remote::Server { auth, .. } if auth.token == "tok")
    );
    crate::app::together::forget_token("127.0.0.1:9", "dev1");
    assert_eq!(
        a.media_remote(),
        crate::app::files::Remote::SignedOut("127.0.0.1:9".into())
    );
}

#[test]
fn sharing_a_folder_brings_earlier_pictures_along() {
    let mut a = app();
    a.add_media(false, Some(picture_file("before.png", 24, 24)), None);
    let files = until_ready(&mut a, false);
    let ch = a.data.channels[0].id;
    a.apply_chat(vec![ChatAction::Send {
        ch,
        text: "from before sharing".into(),
        parent: None,
        files: files.clone(),
    }]);
    let folder = temp_home("share-later");
    a.share_here(&folder.to_string_lossy()).unwrap();
    let dest = folder.join("files").join(&files[0].file);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !dest.is_file() && std::time::Instant::now() < end {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        dest.is_file(),
        "the picture sent earlier is in the shared folder"
    );
}

#[test]
fn a_failed_upload_for_a_sent_message_is_tried_again_and_not_cancelled() {
    let mut a = app();
    a.add_media(false, Some(picture_file("again.png", 16, 16)), None);
    let att = until_ready(&mut a, false).remove(0);
    let sha = att.file.clone();
    let remote = crate::app::files::Remote::Server {
        addr: "127.0.0.1:9".into(),
        auth: wb_net::Auth {
            device: "d".into(),
            token: "t".into(),
        },
    };
    a.files.upload_soon(&att, &remote);
    // removing the same file from a message box doesn't stop it
    a.files.cancel_upload(&sha);
    a.files.settle(&remote, |f| {
        matches!(f.state(&sha), Some(crate::app::files::Transfer::Failed(_)))
            && f.upload_tries(&sha) == Some((1, true))
    });
    a.files.retry_due(&sha);
    a.files.settle(&remote, |f| {
        f.upload_tries(&sha).is_some_and(|(n, _)| n == 2)
    });
}

#[test]
fn a_picture_added_before_sharing_goes_up_when_the_message_is_sent() {
    let mut a = app();
    a.add_media(false, Some(picture_file("early.png", 16, 16)), None);
    let files = until_ready(&mut a, false);
    let sha = files[0].file.clone();
    // now the workspace is on a server this computer is signed in to
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-early".into(),
        tag: 1,
        server: Some("127.0.0.1:9".into()),
    });
    crate::app::together::save_token("127.0.0.1:9", "dev-early", "tok");
    let ch = a.data.channels[0].id;
    a.apply_chat(vec![ChatAction::Send {
        ch,
        text: "made before sharing".into(),
        parent: None,
        files,
    }]);
    crate::app::together::forget_token("127.0.0.1:9", "dev-early");
    assert!(
        a.files.upload_tries(&sha).is_some(),
        "it is sent to the server (and tried again until it's there)"
    );
}

#[test]
fn file_errors_are_shown_in_the_interface_language() {
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    let shown = App::error_text("Notes is a folder.");
    let net = App::error_text("Couldn't reach the server: Connection refused (os error 61)");
    let plain = App::error_text("The server doesn't have this file.");
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert_eq!(shown, "Notes은(는) 폴더입니다.");
    // the system's English words are left out
    assert_eq!(
        net,
        "지금은 팀 서버에 연결할 수 없습니다. 저절로 다시 시도합니다."
    );
    assert_eq!(plain, "서버에 이 파일이 없습니다.");
    assert_eq!(
        App::error_text("Couldn't put a.png in the shared folder: full"),
        "Couldn't put a.png in the shared folder: full"
    );
}

#[test]
fn restoring_a_channel_puts_its_pictures_back_where_the_team_gets_them() {
    let mut a = app();
    let folder = temp_home("trash-reupload");
    a.share_here(&folder.to_string_lossy()).unwrap();
    a.add_media(false, Some(picture_file("trash.png", 24, 24)), None);
    let files = until_ready(&mut a, false);
    let ch = a.data.channels[0].id;
    a.apply_chat(vec![ChatAction::Send {
        ch,
        text: "with a picture".into(),
        parent: None,
        files: files.clone(),
    }]);
    let dest = folder.join("files").join(&files[0].file);
    let wait = |ok: &dyn Fn() -> bool| {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !ok() && std::time::Instant::now() < end {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    };
    wait(&|| dest.is_file());
    assert!(dest.is_file());
    // the channel goes to Trash, and the shared copy of its picture is cleaned up
    let gone = a.data.channels.remove(0);
    a.data.extra.trash.push(TrashItem {
        id: 990_001,
        kind: "Channel".into(),
        title: format!("#{}", gone.name),
        deleted: now_ts(),
        payload: serde_json::to_string(&gone).unwrap(),
    });
    assert_eq!(
        crate::app::files::trash_attachments(&a.data.extra.trash.last().unwrap().payload)
            .iter()
            .map(|x| x.file.clone())
            .collect::<Vec<_>>(),
        vec![files[0].file.clone()]
    );
    std::fs::remove_file(&dest).unwrap();
    let i = a.data.extra.trash.len() - 1;
    a.restore_trash(i);
    assert!(a.data.channels.iter().any(|c| c.id == ch));
    wait(&|| dest.is_file());
    assert!(dest.is_file(), "the picture is back in the shared folder");
}

fn dead_server() -> crate::app::files::Remote {
    crate::app::files::Remote::Server {
        addr: "127.0.0.1:9".into(),
        auth: wb_net::Auth {
            device: "d".into(),
            token: "t".into(),
        },
    }
}

#[test]
fn waiting_uploads_are_kept_on_disk_and_carry_on_after_a_restart() {
    let mut a = app();
    a.add_media(false, Some(picture_file("later.png", 16, 16)), None);
    let att = until_ready(&mut a, false).remove(0);
    let sha = att.file.clone();
    let remote = dead_server();
    a.files.upload_soon(&att, &remote);
    a.files.tick(None, &remote);
    assert!(
        a.files.dir().join(".uploads.json").is_file(),
        "written down"
    );
    // the app quits; the next start reads the list and sends it again
    let dir = a.files.dir().to_path_buf();
    drop(a);
    let mut f = crate::app::files::Files::new(dir.clone());
    assert!(f.is_waiting(&sha));
    f.settle(&remote, |f| {
        matches!(f.state(&sha), Some(crate::app::files::Transfer::Failed(_)))
            && f.upload_tries(&sha) == Some((1, true))
    });
    // meant for another server: it waits for that one
    let other = crate::app::files::Remote::Server {
        addr: "127.0.0.1:10".into(),
        auth: wb_net::Auth {
            device: "d".into(),
            token: "t".into(),
        },
    };
    let mut g = crate::app::files::Files::new(dir);
    g.tick(None, &other);
    assert!(g.is_waiting(&sha));
    assert_eq!(g.state(&sha), None, "not sent to the wrong server");
}

#[test]
fn sharing_brings_task_attachments_along_too() {
    let mut a = app();
    a.add_media(false, Some(picture_file("task.png", 16, 16)), None);
    let att = until_ready(&mut a, false).remove(0);
    a.chat.files.clear();
    let tid = a.data.tasks[0].id;
    a.data.tasks[0].attachments.push(att.clone());
    // and one in a deleted message in the Trash
    a.add_media(false, Some(picture_file("trash.png", 18, 18)), None);
    let gone = until_ready(&mut a, false).remove(0);
    a.chat.files.clear();
    a.data.extra.trash.push(TrashItem {
        id: 1,
        kind: "message".into(),
        title: "m".into(),
        deleted: 1,
        payload: serde_json::json!({"text": "gone", "attachments": [gone.clone()]}).to_string(),
    });
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-task-share".into(),
        tag: 1,
        server: Some("127.0.0.1:9".into()),
    });
    crate::app::together::save_token("127.0.0.1:9", "dev-task-share", "tok");
    a.share_cached_files();
    crate::app::together::forget_token("127.0.0.1:9", "dev-task-share");
    assert!(a.files.is_waiting(&att.file), "the task's picture goes up");
    assert!(a.files.is_waiting(&gone.file), "and the one in the Trash");
    let _ = tid;
}

#[test]
fn a_task_attachment_waiting_for_a_sign_in_goes_up_once_there_is_one() {
    let mut a = app();
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-task-wait".into(),
        tag: 1,
        server: Some("127.0.0.1:9".into()),
    });
    let tid = a.data.tasks[0].id;
    a.add_task_file(tid, picture_file("plan.png", 20, 20));
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while a.task_files[0].1.att.is_none() && std::time::Instant::now() < end {
        a.files_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let att = a.task_files[0].1.att.clone().unwrap();
    assert_eq!(
        a.task_files[0].1.transfer(&a.files),
        Some(crate::app::files::Transfer::Failed(
            crate::app::files::NOT_SIGNED_IN.into()
        ))
    );
    assert!(a.files.is_waiting(&att.file), "kept for later");
    crate::app::together::save_token("127.0.0.1:9", "dev-task-wait", "tok");
    // signed in: it is sent (the dead server refuses, so it fails and is
    // tried again later instead of waiting for a sign-in)
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        a.files_tick(None);
        let t = a.task_files[0].1.transfer(&a.files);
        if matches!(&t, Some(crate::app::files::Transfer::Failed(e)) if e != crate::app::files::NOT_SIGNED_IN)
        {
            break;
        }
        assert!(std::time::Instant::now() < end, "never sent: {:?}", t);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    crate::app::together::forget_token("127.0.0.1:9", "dev-task-wait");
    assert!(a.files.upload_tries(&att.file).is_some());
}

fn make_clip(name: &str) -> Option<std::path::PathBuf> {
    let ffmpeg = ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())?;
    let dir = temp_home("clip");
    std::fs::create_dir_all(&dir).unwrap();
    let clip = dir.join(name);
    let ok = std::process::Command::new(ffmpeg)
        .args([
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=2:size=1280x720:rate=30",
            "-pix_fmt",
            "yuv420p",
            "-c:v",
            "libx264",
        ])
        .arg(&clip)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok);
    Some(clip)
}

#[test]
fn a_video_goes_with_a_small_still_and_others_show_that_first() {
    let Some(clip) = make_clip("Trip.mp4") else {
        eprintln!("skipped: ffmpeg isn't installed");
        return;
    };
    let folder = temp_home("still-share");
    let mut a = app();
    a.share_here(&folder.to_string_lossy()).unwrap();
    a.add_media(false, Some(clip), None);
    let v = until_ready(&mut a, false).remove(0);
    let still = v.thumb.clone().expect("a still is made");
    let jpeg = std::fs::read(a.files.local(&still).unwrap()).unwrap();
    assert_eq!(&jpeg[..2], &[0xff, 0xd8], "a JPEG");
    assert!(jpeg.len() < 200_000, "small: {} bytes", jpeg.len());
    assert!(folder.join("files").join(&still).is_file(), "shared too");
    // it's saved with the message, and older files without it still load
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json["thumb"], serde_json::json!(still));
    let old: Attachment = serde_json::from_value(
        serde_json::json!({"file": v.file, "name": "a.mp4", "kind": "Video"}),
    )
    .unwrap();
    assert_eq!(old.thumb, None);
    assert!(!serde_json::to_string(&old).unwrap().contains("thumb"));
    assert_eq!(
        crate::app::files::shown(&old).file,
        v.file,
        "old ones show as before"
    );
    // another computer shows the still without fetching the video
    let mut bf = crate::app::files::Files::new(temp_home("still-b"));
    let remote = crate::app::files::Remote::Folder(folder.clone());
    let shown = crate::app::files::shown(&v);
    assert_eq!(shown.file, still);
    bf.settle(&remote, |f| {
        f.want(&shown, crate::app::files::THUMB);
        f.decoded(&still, crate::app::files::THUMB)
    });
    let s = bf.texture(&still, crate::app::files::THUMB);
    let _ = s;
    assert!(bf.local(&v.file).is_none(), "the video stays on the server");
}

#[test]
fn a_video_and_its_still_both_have_to_be_up_before_the_message_goes() {
    let a = app();
    let v = Attachment {
        file: "a".repeat(64),
        name: "v.mp4".into(),
        kind: MediaKind::Video,
        thumb: Some("b".repeat(64)),
        ..Default::default()
    };
    let parts = crate::app::files::parts(&v);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[1].kind, MediaKind::Image);
    let p = crate::app::files::Pending {
        local: 1,
        name: "v.mp4".into(),
        att: Some(v.clone()),
        error: None,
    };
    a.files.upload(&v, &crate::app::files::Remote::None);
    assert!(p.ready(&a.files));
    // a picture never has a still, whatever the field says
    let pic = Attachment {
        kind: MediaKind::Image,
        ..v
    };
    assert_eq!(crate::app::files::parts(&pic).len(), 1);
}

fn old_file(p: &std::path::Path, days: u64) {
    let f = std::fs::File::options().write(true).open(p).unwrap();
    f.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(days * 24 * 3600))
        .unwrap();
}

#[test]
fn the_cache_drops_unused_files_after_a_while_and_old_team_files_past_its_size() {
    use crate::app::files::clean_cache;
    use std::collections::HashSet;
    let dir = temp_home("clean-cache");
    std::fs::create_dir_all(&dir).unwrap();
    let sha = |c: char| c.to_string().repeat(64);
    let put = |c: char, bytes: usize, days: u64| {
        let p = dir.join(sha(c));
        std::fs::write(&p, vec![0u8; bytes]).unwrap();
        old_file(&p, days);
        p
    };
    put('a', 100, 30); // nobody refers to it, old: goes
    put('b', 100, 1); // nobody refers to it, new: stays (a box may send it)
    put('c', 100, 30); // in the Trash of a workspace: stays
    put('d', 100, 30); // waiting to go up: stays
    put('e', 1000, 20); // a shared workspace's, least recently used: goes past the cap
    put('f', 1000, 2); // a shared workspace's, used lately: stays
    put('9', 1000, 40); // a workspace only on this computer: never goes
    std::fs::write(dir.join(format!("{}.mov", sha('a'))), b"x").unwrap();
    std::fs::write(dir.join(".uploads.json"), b"[]").unwrap();
    std::fs::write(dir.join(".task-files.json"), b"[]").unwrap();
    old_file(&dir.join(".task-files.json"), 3);
    let local: HashSet<String> = [sha('c'), sha('9')].into_iter().collect();
    let shared: HashSet<String> = [sha('e'), sha('f')].into_iter().collect();
    let keep: HashSet<String> = [sha('d')].into_iter().collect();
    let space = |shared: bool, all: HashSet<String>| crate::app::files::SpaceRefs {
        shared,
        all,
        trash_only: HashSet::new(),
    };
    let removed = clean_cache(
        &dir,
        &[space(false, local), space(true, shared)],
        &keep,
        std::time::SystemTime::now(),
        crate::app::files::CACHE_GRACE,
        2500,
    );
    let mut removed = removed;
    removed.sort();
    assert_eq!(removed, vec![sha('a'), sha('e')]);
    for c in ['b', 'c', 'd', 'f', '9'] {
        assert!(dir.join(sha(c)).is_file(), "{} stays", c);
    }
    assert!(
        !dir.join(format!("{}.mov", sha('a'))).exists(),
        "its link goes too"
    );
    assert!(dir.join(".uploads.json").is_file(), "the backlog stays");
    assert!(
        dir.join(".task-files.json").is_file(),
        "and the task attachments on their way"
    );
}

#[test]
fn the_shared_folder_drops_files_nobody_refers_to_once_they_are_a_month_old() {
    use crate::app::files::{clean_shared_folder, folder_refs};
    let folder = temp_home("clean-folder");
    let files = folder.join("files");
    std::fs::create_dir_all(&files).unwrap();
    let sha = |c: char| c.to_string().repeat(64);
    for (c, days) in [('a', 40), ('b', 40), ('c', 5), ('d', 40)] {
        let p = files.join(sha(c));
        std::fs::write(&p, b"x").unwrap();
        old_file(&p, days);
    }
    // two members' copies: one refers to b (a message), the other to d (a task)
    std::fs::write(
        folder.join("one.workboard.json"),
        serde_json::json!({"channels": [{"messages": [{"attachments": [{"file": sha('b'), "name": "b.png"}]}]}]}).to_string(),
    )
    .unwrap();
    std::fs::write(
        folder.join("two.workboard.json"),
        serde_json::json!({"tasks": [{"attachments": [{"file": sha('d'), "name": "d.png"}]}]})
            .to_string(),
    )
    .unwrap();
    // a folder that has been cleaned up before (none named lately)
    std::fs::write(files.join(crate::app::files::FOLDER_NAMED), "{}").unwrap();
    let refs = folder_refs(&folder);
    let removed = clean_shared_folder(
        &files,
        &refs,
        std::time::SystemTime::now(),
        crate::app::files::FOLDER_GRACE,
    );
    assert_eq!(removed, vec![sha('a')], "old and unused only");
    assert!(files.join(sha('b')).is_file() && files.join(sha('c')).is_file());
    assert!(files.join(sha('d')).is_file());
    // a member's copy that can't be read (half synced): nothing is touched
    std::fs::write(folder.join("three.workboard.json"), "{\"tasks\": [").unwrap();
    let b = files.join(sha('b'));
    std::fs::write(folder.join("one.workboard.json"), "{}").unwrap();
    assert!(folder_refs(&folder).is_none());
    assert!(clean_shared_folder(
        &files,
        &folder_refs(&folder),
        std::time::SystemTime::now(),
        crate::app::files::FOLDER_GRACE
    )
    .is_empty());
    assert!(b.is_file());
}

#[test]
fn every_workspace_on_this_computer_counts_for_the_cache() {
    let dir = temp_home("clean-spaces");
    std::fs::create_dir_all(&dir).unwrap();
    let sha = |c: char| c.to_string().repeat(64);
    std::fs::write(
        dir.join("data.json"),
        serde_json::json!({"extra": {"shared": null, "trash": [{"payload": serde_json::json!({"attachments": [{"file": sha('a'), "name": "x.png"}]}).to_string()}]}}).to_string(),
    )
    .unwrap();
    std::fs::write(
        dir.join("team.json"),
        serde_json::json!({"extra": {"shared": {"device": "d", "tag": 1}}, "tasks": [{"attachments": [{"file": sha('b'), "name": "y.mp4", "kind": "Video", "thumb": sha('c')}]}]}).to_string(),
    )
    .unwrap();
    std::fs::write(
        dir.join("broken.json"),
        format!("{{ \"file\": \"{}\"", sha('d')),
    )
    .unwrap();
    let spaces = crate::app::files::workspaces_on_disk(&dir);
    let find = |c: char| {
        spaces
            .iter()
            .find(|s| s.all.contains(&sha(c)))
            .map(|s| s.shared)
    };
    assert_eq!(find('a'), Some(false), "the Trash counts");
    assert_eq!(find('b'), Some(true));
    assert_eq!(find('c'), Some(true), "a video's still counts");
    assert_eq!(
        find('d'),
        Some(false),
        "an unreadable file counts by its text"
    );
}

#[test]
fn a_task_attachment_still_going_up_at_quit_joins_its_task_after_a_restart() {
    let mut a = app();
    // a server this computer isn't signed in to: it can't go up yet
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-task-restart".into(),
        tag: 1,
        server: Some("127.0.0.1:9".into()),
    });
    let tid = a.data.tasks[0].id;
    a.add_task_file(tid, picture_file("restart.png", 20, 22));
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while a.task_files[0].1.att.is_none() && std::time::Instant::now() < end {
        a.files_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    a.files_tick(None);
    let att = a.task_files[0].1.att.clone().unwrap();
    let dir = a.files.dir().to_path_buf();
    let saved = crate::app::taskfields::read_task_files(&dir);
    assert_eq!(saved.len(), 1, "written down");
    assert_eq!((saved[0].task, &saved[0].att.file), (tid, &att.file));
    // and one from another workspace on this computer
    let mut other = saved[0].clone();
    other.ws = "other.json".into();
    other.task = 424_242;
    let mut both = saved.clone();
    both.push(other);
    std::fs::write(
        dir.join(crate::app::taskfields::TASK_FILES),
        serde_json::to_string(&both).unwrap(),
    )
    .unwrap();
    // the app quits before it goes up; the next start (signed in this time,
    // here: no server, so it is ready at once) attaches it to the task
    let mut data = a.data.clone();
    data.extra.shared = None;
    drop(a);
    let mut b = App::build(None, Some(data));
    b.files = crate::app::files::Files::new(dir.clone());
    assert!(b.task(tid).unwrap().attachments.is_empty());
    b.files_tick(None);
    b.files_tick(None);
    let t = b.task(tid).unwrap();
    assert_eq!(t.attachments.len(), 1, "joined its task");
    assert_eq!(t.attachments[0].file, att.file);
    assert!(b.task_files.is_empty());
    let left = crate::app::taskfields::read_task_files(&dir);
    assert_eq!(
        left.len(),
        1,
        "the other workspace's one stays written down"
    );
    assert_eq!(left[0].ws, "other.json");
}

#[test]
fn the_cache_never_drops_files_only_a_trash_refers_to_to_stay_under_its_size() {
    use crate::app::files::{clean_cache, SpaceRefs};
    use std::collections::HashSet;
    let dir = temp_home("clean-trash");
    std::fs::create_dir_all(&dir).unwrap();
    let sha = |c: char| c.to_string().repeat(64);
    for (c, days) in [('a', 50), ('b', 40), ('c', 30)] {
        let p = dir.join(sha(c));
        std::fs::write(&p, vec![0u8; 1000]).unwrap();
        old_file(&p, days);
    }
    // a shared workspace: a (least recently used) only in its Trash, b in
    // a message, c in another workspace's message but this one's Trash
    let ws = serde_json::json!({
        "extra": {"shared": {"device": "d", "tag": 1}, "trash": [
            {"payload": serde_json::json!({"attachments": [{"file": sha('a'), "name": "a.png"}]}).to_string()},
            {"payload": serde_json::json!({"attachments": [{"file": sha('c'), "name": "c.png"}]}).to_string()}
        ]},
        "channels": [{"messages": [{"attachments": [{"file": sha('b'), "name": "b.png"}]}]}]
    });
    let refs = SpaceRefs::of(&ws);
    assert!(refs.shared);
    assert_eq!(refs.all.len(), 3);
    let only: HashSet<String> = [sha('a'), sha('c')].into_iter().collect();
    assert_eq!(refs.trash_only, only);
    let other = SpaceRefs {
        shared: true,
        all: [sha('c')].into_iter().collect(),
        trash_only: HashSet::new(),
    };
    let removed = clean_cache(
        &dir,
        &[refs, other],
        &HashSet::new(),
        std::time::SystemTime::now(),
        crate::app::files::CACHE_GRACE,
        1000,
    );
    // over the cap: the old Trash-only file stays; the others can be fetched
    // again from where the live messages are
    let mut removed = removed;
    removed.sort();
    assert_eq!(removed, vec![sha('b'), sha('c')]);
    assert!(dir.join(sha('a')).is_file());
}

#[test]
fn the_shared_folder_keeps_a_deleted_messages_file_while_it_was_named_lately() {
    use crate::app::files::{clean_shared_folder, FOLDER_GRACE, FOLDER_NAMED, FOLDER_NAMED_GRACE};
    use std::collections::HashSet;
    let folder = temp_home("clean-named");
    let files = folder.join("files");
    std::fs::create_dir_all(&files).unwrap();
    let sha = |c: char| c.to_string().repeat(64);
    let day = std::time::Duration::from_secs(86_400);
    let now = std::time::SystemTime::now();
    for c in ['a', 'b'] {
        let p = files.join(sha(c));
        std::fs::write(&p, b"x").unwrap();
        old_file(&p, 100);
    }
    let named: Option<HashSet<String>> = Some([sha('a')].into_iter().collect());
    let nothing: Option<HashSet<String>> = Some(HashSet::new());
    // the first clean up only starts watching
    assert!(clean_shared_folder(&files, &named, now, FOLDER_GRACE).is_empty());
    assert!(files.join(FOLDER_NAMED).is_file());
    // a's message is deleted (to someone's Trash): it stays a while
    let soon = now + day * 40;
    assert!(clean_shared_folder(&files, &nothing, soon, FOLDER_GRACE).is_empty());
    assert!(files.join(sha('a')).is_file());
    let later = now + FOLDER_NAMED_GRACE + day;
    let mut gone = clean_shared_folder(&files, &nothing, later, FOLDER_GRACE);
    gone.sort();
    assert_eq!(gone, vec![sha('a'), sha('b')]);
    // a record cut short (or a sync conflict) deletes nothing and starts again
    let p = files.join(sha('c'));
    std::fs::write(&p, b"x").unwrap();
    old_file(&p, 100);
    std::fs::write(files.join(FOLDER_NAMED), "{\"cc").unwrap();
    assert!(clean_shared_folder(&files, &nothing, later, FOLDER_GRACE).is_empty());
    assert!(p.is_file());
    let record: std::collections::BTreeMap<String, i64> =
        serde_json::from_str(&std::fs::read_to_string(files.join(FOLDER_NAMED)).unwrap()).unwrap();
    assert!(record.contains_key(&sha('c')), "rebuilt");
}

/// The short lowercase answers of the team server's file route (which the
/// app shows as "The server said 500 can't write files") read in Korean
/// too, so a new one without a translation is caught.
#[test]
fn server_file_answers_are_shown_in_the_interface_language() {
    let src = include_str!("../../../server/src/files.rs");
    let src = src.split("#[cfg(test)]").next().unwrap();
    let mut bodies = vec![];
    for (i, _) in src.match_indices("StatusCode::") {
        let rest = &src[i + "StatusCode::".len()..];
        let name_end = rest
            .find(|c: char| !(c.is_ascii_uppercase() || c == '_'))
            .unwrap_or(0);
        let after = rest[name_end..].trim_start();
        let Some(after) = after.strip_prefix(',') else {
            continue;
        };
        let Some(lit) = after.trim_start().strip_prefix('"') else {
            continue;
        };
        let body = lit.split('"').next().unwrap();
        if !bodies.contains(&body) {
            bodies.push(body);
        }
    }
    assert!(bodies.len() >= 6, "found the answers: {:?}", bodies);
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    let shown: Vec<(String, String)> = bodies
        .iter()
        .map(|b| {
            let e = format!("The server said 500 {}", b);
            (b.to_string(), App::error_text(&e))
        })
        .collect();
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let english: Vec<&(String, String)> = shown
        .iter()
        .filter(|(_, ko)| ko.chars().any(|c| c.is_ascii_alphabetic()))
        .collect();
    assert!(english.is_empty(), "no Korean for: {:#?}", english);
    // and in English the server's words stay as they are
    assert_eq!(
        App::error_text("The server said 500 can't write files"),
        "The server said 500 can't write files"
    );
}
