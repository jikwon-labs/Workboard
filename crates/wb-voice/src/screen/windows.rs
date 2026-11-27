// Screen sharing on Windows and other systems (see screen.rs): the
// screens, the windows and the picture all come from libwebrtc's capturer.

use super::*;
use livekit::webrtc::desktop_capturer::{
    DesktopCaptureSourceType, DesktopCapturer, DesktopCapturerOptions,
};

/// Windows asks no permission to capture the screen.
pub fn allowed() -> bool {
    true
}
pub fn ask() -> bool {
    true
}

/// What libwebrtc offers of one kind: (id, title).
fn list(ty: DesktopCaptureSourceType) -> Option<Vec<(u64, String)>> {
    let cap = DesktopCapturer::new(DesktopCapturerOptions::new(ty))?;
    Some(
        cap.get_source_list()
            .into_iter()
            .map(|s| (s.id(), s.title()))
            .collect(),
    )
}

pub fn sources() -> Result<Vec<Source>, ScreenTrouble> {
    let _com = os::Com::init();
    let screens = list(DesktopCaptureSourceType::Screen)
        .ok_or_else(|| ScreenTrouble::Failed("Couldn't look for the screens to share.".into()))?;
    // on Windows a screen's id is its display device number, and it has no title
    let primary = os::primary_screen();
    let mut out: Vec<Source> = screens
        .into_iter()
        .enumerate()
        .map(|(n, (id, title))| Source {
            kind: SourceKind::Display,
            id: id as u32,
            name: if title.trim().is_empty() {
                format!("Display {}", n + 1)
            } else {
                title
            },
            main: primary == Some(id as u32),
        })
        .collect();
    if !out.iter().any(|s| s.main) {
        if let Some(first) = out.first_mut() {
            first.main = true;
        }
    }
    // windows of other apps with a title (libwebrtc leaves out hidden
    // windows, tool windows and this app's own)
    let mut wins: Vec<Source> = list(DesktopCaptureSourceType::Window)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, title)| !title.trim().is_empty())
        .map(|(id, name)| Source {
            kind: SourceKind::Window,
            id: id as u32,
            name,
            main: false,
        })
        .collect();
    wins.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.extend(wins);
    out.sort_by_key(|s| (s.kind == SourceKind::Window, !s.main));
    Ok(out)
}

pub fn capture(
    share: Share,
    out: &Out,
    limits: &Limits,
    stop: &AtomicBool,
) -> Result<(), ScreenTrouble> {
    let _com = os::Com::init();
    run_webrtc(share, out, limits, stop)
}
