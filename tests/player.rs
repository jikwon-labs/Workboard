// The in-app video player (src/media.rs) with a real clip. AVPlayer loads
// and delivers frames through the main thread's run loop, which libtest's
// threads don't have, so this runs as its own program on the main thread
// (harness = false in Cargo.toml). Skipped without ffmpeg to make the clip.

#[allow(dead_code)]
#[path = "../src/media.rs"]
mod media;

// media.rs shows its file-picker text through the app's translation; the
// player needs none of it
#[allow(dead_code)]
mod i18n {
    pub fn tr(s: &str) -> &str {
        s
    }
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopDefaultMode: *const std::ffi::c_void;
    fn CFRunLoopRunInMode(mode: *const std::ffi::c_void, secs: f64, once: u8) -> i32;
}

/// Let the main run loop work for a while (as the app's event loop does).
#[cfg(target_os = "macos")]
fn spin(secs: f64) {
    unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, secs, 0) };
}

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    use std::path::Path;
    use std::time::{Duration, Instant};
    let Some(ffmpeg) = ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"].into_iter().find(|p| Path::new(p).exists()) else {
        eprintln!("player: skipped, ffmpeg isn't installed (brew install ffmpeg)");
        return;
    };
    let dir = std::env::temp_dir().join(format!("wb-player-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let clip = dir.join("clip.mp4");
    // 3 seconds of a test pattern with a tone
    let ok = std::process::Command::new(ffmpeg)
        .args(["-loglevel", "error", "-f", "lavfi", "-i", "testsrc=duration=3:size=320x240:rate=30", "-f", "lavfi", "-i", "sine=frequency=440:duration=3", "-pix_fmt", "yuv420p", "-c:v", "libx264", "-c:a", "aac", "-shortest"])
        .arg(&clip)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "ffmpeg made the clip");

    let mut p = media::Player::new(&clip, 160, 120).expect("a player");
    p.set_muted(true);
    assert!(p.muted());
    p.play();
    let until = Instant::now() + Duration::from_secs(10);
    let mut frames = vec![];
    while frames.len() < 5 && Instant::now() < until {
        spin(0.05);
        if let Some(f) = p.frame() {
            frames.push(f);
        }
    }
    assert!(frames.len() >= 5, "frames keep coming while it plays (got {})", frames.len());
    assert!(frames.iter().all(|f| (f.w, f.h) == (160, 120) && f.px.len() == 160 * 120 * 4), "scaled to the asked size");
    assert!(frames[0].px.chunks(4).any(|c| c[0] > 100 || c[1] > 100 || c[2] > 100), "not black");
    assert!(frames.first().map(|f| &f.px) != frames.last().map(|f| &f.px), "the picture moves");
    assert!(p.playing());
    assert!((p.duration() - 3.0).abs() < 0.1, "duration {}", p.duration());

    p.seek(2.0);
    spin(0.5);
    assert!(p.position() >= 1.9, "seeked to {}", p.position());
    p.pause();
    spin(0.1);
    assert!(!p.playing());
    let at = p.position();
    spin(0.4);
    assert!((p.position() - at).abs() < 0.05, "stays put while paused");

    // playing to the end and pressing play again starts over
    p.seek(2.9);
    p.play();
    let until = Instant::now() + Duration::from_secs(5);
    while p.playing() && Instant::now() < until {
        spin(0.05);
    }
    assert!(!p.playing(), "stops at the end");
    p.play();
    spin(0.3);
    assert!(p.position() < 1.0, "started over at {}", p.position());
    drop(p);
    let _ = std::fs::remove_dir_all(&dir);
    println!("player: ok");
}
