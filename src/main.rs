#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use worktree_hub::{APP_NAME, app, desktop, icon};

fn main() -> eframe::Result {
    // `worktree-hub --dump-icon <file.png> [size]` is used by the packaging
    // scripts so the desktop/AppImage icon is the same artwork as the window icon.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "--dump-icon" {
        let path = args
            .get(2)
            .map(String::as_str)
            .unwrap_or("worktree-hub.png");
        let size: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(512);
        match icon::write_png(path, size) {
            Ok(()) => println!("icon written: {path} ({size}x{size})"),
            Err(err) => {
                eprintln!("failed to write icon: {err}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // `worktree-hub --terminals <path>...` opens straight into the multi-terminal
    // view with one pane per path — handy from scripts and launchers.
    let open_terminals: Vec<std::path::PathBuf> = match args.iter().position(|a| a == "--terminals")
    {
        Some(index) => args[index + 1..]
            .iter()
            .map(std::path::PathBuf::from)
            .collect(),
        None => Vec::new(),
    };

    // An AppImage is installed nowhere, so the desktop has nothing to take the
    // window icon from until the app says where it lives.
    desktop::integrate();

    let rgba = icon::rgba(256);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id("worktree-hub")
            .with_inner_size([1220.0, 780.0])
            .with_min_inner_size([940.0, 600.0])
            // The window can be see-through; how much is a setting, applied
            // every frame through `App::clear_color`.
            .with_transparent(true)
            .with_icon(egui::IconData {
                rgba,
                width: 256,
                height: 256,
            }),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, &open_terminals)))),
    )
}
