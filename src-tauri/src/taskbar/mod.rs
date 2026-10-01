//! The taskbar button of the main window: progress, overlay badge, flashing. The page sends
//! signals, `app_state::taskbar::resolve` picks the winner, this applies it (R-638).

use app_state::taskbar::{Appearance, Bar, Flash, ICON_SIZE, overlay_rgba};
use tauri::UserAttentionType;
use tauri::image::Image;
use tauri::window::{ProgressBarState, ProgressBarStatus};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
use stub as platform;

pub fn apply(window: &tauri::Window, shown: &Appearance) -> tauri::Result<()> {
    let (status, progress) = match shown.bar {
        Bar::None => (ProgressBarStatus::None, None),
        Bar::Normal(value) => (ProgressBarStatus::Normal, Some(u64::from(value))),
        Bar::Indeterminate => (ProgressBarStatus::Indeterminate, None),
        Bar::Error => (ProgressBarStatus::Error, Some(100)),
    };
    window.set_progress_bar(ProgressBarState {
        status: Some(status),
        progress,
    })?;

    let icon = overlay_rgba(shown.overlay).map(|rgba| Image::new_owned(rgba, ICON_SIZE, ICON_SIZE));
    platform::set_overlay(window, icon)?;

    // Never flash a window the user is looking at; Windows stops a persistent flash on focus.
    if let Some(flash) = shown.flash
        && !window.is_focused().unwrap_or(false)
    {
        let kind = match flash {
            Flash::Short => UserAttentionType::Informational,
            Flash::Persistent => UserAttentionType::Critical,
        };
        window.request_user_attention(Some(kind))?;
    }
    Ok(())
}
