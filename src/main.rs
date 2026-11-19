// the Windows app is a window, not a console program: no black console
// window beside it (debug builds keep it for printed messages)
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod filters;
mod i18n;
mod links;
mod model;
mod notifier;
mod os;
use wb_sync as sync;
mod media;
mod theme;
mod timefmt;
mod updater;
mod widgets;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot = Some(PathBuf::from(&args[i + 1]));
            i += 1;
        }
        i += 1;
    }
    let shot = screenshot.is_some();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Workboard")
        // no runtime icon: otherwise eframe puts its own "e" icon in the Dock over the bundle's AppIcon
        .with_icon(egui::IconData::default())
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([900.0, 560.0])
        // like the original: no separate title bar, window buttons sit in the header
        .with_fullsize_content_view(true)
        .with_title_shown(false)
        .with_titlebar_shown(false);
    if shot {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    #[cfg(target_os = "macos")]
    if shot {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }

    links::listen();
    eframe::run_native(
        "Workboard",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot)))),
    )
}
