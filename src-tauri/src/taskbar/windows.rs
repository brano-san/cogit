//! Windows: `ITaskbarList3` through Tauri — `SetOverlayIcon` here, `SetProgressState` and
//! `FlashWindowEx` in `apply`. No COM of our own was needed.

use tauri::image::Image;

pub fn set_overlay(window: &tauri::Window, icon: Option<Image<'_>>) -> tauri::Result<()> {
    window.set_overlay_icon(icon)
}
