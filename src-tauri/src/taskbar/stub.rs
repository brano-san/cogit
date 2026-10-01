//! Not implemented. Seams for the other platforms:
//! - Linux: `com.canonical.Unity.LauncherEntry` (count badge) over D-Bus, and
//!   `_NET_WM_STATE_DEMANDS_ATTENTION` for the flash; Tauri's progress bar already covers
//!   the bar where `libunity` exists.
//! - macOS: `NSDockTile.badgeLabel` (`Window::set_badge_label`) and
//!   `NSApplication.requestUserAttention`, which `request_user_attention` already wraps.

use tauri::image::Image;

pub fn set_overlay(_window: &tauri::Window, _icon: Option<Image<'_>>) -> tauri::Result<()> {
    Ok(())
}
