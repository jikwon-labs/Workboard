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
mod single;
mod startup;
use wb_sync as sync;
mod media;
mod theme;
mod timefmt;
mod updater;
mod widgets;

use std::path::PathBuf;

/// macOS: none, or eframe puts its own "e" icon in the Dock over the
/// bundle's AppIcon. Windows: the logo, for the title bar and the taskbar
/// button (the exe's own icon, from build.rs, is for File Explorer).
fn window_icon() -> egui::IconData {
    #[cfg(windows)]
    if let Ok(icon) =
        eframe::icon_data::from_png_bytes(include_bytes!("../Resources/AppIcon-256.png"))
    {
        return icon;
    }
    egui::IconData::default()
}

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
        .with_icon(window_icon())
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
    // Windows, --wgpu: Direct3D through wgpu instead of OpenGL, which virtual
    // machines and PCs without a graphics driver often lack (see Cargo.toml).
    // Not the default yet: it doesn't open on GitHub's GPU-less machines.
    #[cfg(windows)]
    if args.iter().any(|a| a == "--wgpu") {
        options.renderer = eframe::Renderer::Wgpu;
    }
    // Windows has no console to print an error on: say it in a message box
    #[cfg(windows)]
    startup::watch_panics(shot);
    #[cfg(target_os = "macos")]
    if shot {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }

    // started by a Windows update: the copy that installed it ends first,
    // or the one-copy rule below would hand this one over to it
    updater::wait_for_old_copy();
    // Windows: one copy per data folder; a later one hands its link over and ends
    if !shot && single::start() == single::Start::Handed {
        return Ok(());
    }
    links::listen();
    let run = eframe::run_native(
        "Workboard",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot)))),
    );
    if let Err(e) = &run {
        startup::failed(&e.to_string(), shot);
    }
    run
}
