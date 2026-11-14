//! Teaches the desktop about the app when it is running from an AppImage.
//!
//! A Wayland window does not carry its own icon: the compositor takes the app
//! id, looks for a desktop entry with that name among the ones installed on
//! the machine, and draws the icon that entry points at. An AppImage is
//! installed nowhere, so there is no entry to find and the window wears the
//! compositor's fallback mark instead of ours. The fix is to put the entry
//! where the desktop looks — in the user's own data directory — with `Exec`
//! aimed at the very file that is running. The same entry puts the app in the
//! menu, which is what someone who kept the AppImage wanted anyway.

use std::fs;
use std::path::Path;

use crate::icon;

/// Name shared by the app id, the desktop entry and the icon file. The
/// compositor only matches the icon when all three agree.
const ID: &str = "worktree-hub";

/// The entry as it is shipped to packagers, so the menu says the same thing
/// however the app arrived.
const TEMPLATE: &str = include_str!("../assets/worktree-hub.desktop");

/// Writes the desktop entry and the icon for an AppImage run. Does nothing
/// when the app was started any other way: a package manager has already put
/// both in place, and a `cargo run` should leave no trace on the machine.
pub fn integrate() {
    let Ok(appimage) = std::env::var("APPIMAGE") else {
        return;
    };
    if appimage.is_empty() || !Path::new(&appimage).exists() {
        return;
    }
    let Some(dirs) = directories::BaseDirs::new() else {
        return;
    };
    let data = dirs.data_dir();

    let icon_dir = data.join("icons/hicolor/256x256/apps");
    let icon_file = icon_dir.join(format!("{ID}.png"));
    if !icon_file.exists()
        && fs::create_dir_all(&icon_dir).is_ok()
        && let Some(path) = icon_file.to_str()
    {
        let _ = icon::write_png(path, 256);
    }

    // A path with spaces has to be quoted, and the entry has to point at the
    // AppImage wherever the user keeps it — it moves, and so does the entry.
    let entry = TEMPLATE.replace("Exec=worktree-hub", &format!("Exec=\"{appimage}\" %U"));
    let apps = data.join("applications");
    let file = apps.join(format!("{ID}.desktop"));
    let same = fs::read_to_string(&file).is_ok_and(|old| old == entry);
    if !same && fs::create_dir_all(&apps).is_ok() {
        let _ = fs::write(&file, entry);
    }
}
