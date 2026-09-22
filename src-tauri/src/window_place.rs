//! Where the window is allowed to be.
//!
//! `tauri-plugin-window-state` restores the position it saved, and it saves whatever the
//! last machine had. Carry the settings to a laptop and the window opens above the screen
//! with its title bar out of reach (doc/12-risks.md, R-113). Everything here works on
//! physical pixels, which is what both the plugin and the monitor list speak.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    fn right(self) -> i32 {
        self.x + self.w
    }

    fn bottom(self) -> i32 {
        self.y + self.h
    }
}

/// The strip the user drags the window by. Anything less than this on screen and the
/// window cannot be moved with the mouse.
const GRAB_HEIGHT: i32 = 32;
/// How much of that strip has to be reachable. A sliver at the very edge is not enough
/// to aim at.
const GRAB_WIDTH: i32 = 120;

/// How much of the grab strip has to be on screen vertically. A maximized or snapped
/// window deliberately hangs its frame a few pixels past the work area, and demanding the
/// whole strip called every one of those off screen (R-118).
const GRAB_VISIBLE: i32 = 8;

/// Can the title bar be grabbed on any of these monitors?
#[must_use]
pub fn reachable(window: Rect, monitors: &[Rect]) -> bool {
    let strip = Rect {
        y: window.y,
        h: GRAB_HEIGHT,
        ..window
    };
    monitors.iter().any(|screen| {
        let across = strip.right().min(screen.right()) - strip.x.max(screen.x);
        let down = strip.bottom().min(screen.bottom()) - strip.y.max(screen.y);
        across >= GRAB_WIDTH && down >= GRAB_VISIBLE
    })
}

fn overlap(a: Rect, b: Rect) -> i64 {
    let wide = (a.right().min(b.right()) - a.x.max(b.x)).max(0);
    let tall = (a.bottom().min(b.bottom()) - a.y.max(b.y)).max(0);
    i64::from(wide) * i64::from(tall)
}

/// The monitor the window is mostly on, which is the one whose size it has to respect.
fn home(window: Rect, monitors: &[Rect], primary: Rect) -> Rect {
    monitors
        .iter()
        .copied()
        .filter(|screen| overlap(window, *screen) > 0)
        .max_by_key(|screen| overlap(window, *screen))
        .unwrap_or(primary)
}

/// The rectangle to actually use: the saved one when it is usable, the middle of the
/// primary monitor when it is not. Never larger than the monitor it ends up on.
#[must_use]
pub fn place(window: Rect, monitors: &[Rect], primary: Rect) -> Rect {
    let screen = home(window, monitors, primary);
    let sized = Rect {
        w: window.w.min(screen.w),
        h: window.h.min(screen.h),
        ..window
    };
    if reachable(sized, monitors) {
        return sized;
    }
    let fitted = Rect {
        w: window.w.min(primary.w),
        h: window.h.min(primary.h),
        ..window
    };
    Rect {
        x: primary.x + (primary.w - fitted.w) / 2,
        y: primary.y + (primary.h - fitted.h) / 2,
        ..fitted
    }
}

impl From<&tauri::Monitor> for Rect {
    fn from(monitor: &tauri::Monitor) -> Self {
        let area = monitor.work_area();
        Self {
            x: area.position.x,
            y: area.position.y,
            w: i32::try_from(area.size.width).unwrap_or(i32::MAX),
            h: i32::try_from(area.size.height).unwrap_or(i32::MAX),
        }
    }
}

/// Whether a window in this state has a position worth checking at all.
///
/// A maximized window is on a monitor by definition, and its frame deliberately hangs a
/// few pixels past the work area — "correcting" that un-maximizes it. A minimized window
/// sits at about (-32000, -32000), and moving it from there breaks the rectangle Windows
/// restores to. Fullscreen is the operating system's business.
#[must_use]
pub fn should_settle(maximized: bool, minimized: bool, fullscreen: bool) -> bool {
    !maximized && !minimized && !fullscreen
}

/// Moves the window onto a monitor that exists, if it is not on one already. Called
/// **once**, after the state plugin has restored the saved position and before the window
/// is shown. Never in response to a window event: Windows moves and resizes the window
/// itself, and a handler that answers by moving it back is a fight (R-118).
pub fn settle<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    let maximized = window.is_maximized().unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);
    let fullscreen = window.is_fullscreen().unwrap_or(false);
    if !should_settle(maximized, minimized, fullscreen) {
        tracing::debug!(
            maximized,
            minimized,
            fullscreen,
            "window geometry left to Windows"
        );
        return;
    }
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let Ok(monitors) = window.available_monitors() else {
        return;
    };

    let screens: Vec<Rect> = monitors.iter().map(Rect::from).collect();
    let primary = window
        .primary_monitor()
        .ok()
        .flatten()
        .as_ref()
        .map(Rect::from)
        .or_else(|| screens.first().copied());
    let Some(primary) = primary else { return };

    let was = Rect {
        x: position.x,
        y: position.y,
        w: i32::try_from(size.width).unwrap_or(i32::MAX),
        h: i32::try_from(size.height).unwrap_or(i32::MAX),
    };
    let now = place(was, &screens, primary);
    if now == was {
        return;
    }

    tracing::info!(
        from = ?(was.x, was.y, was.w, was.h),
        to = ?(now.x, now.y, now.w, now.h),
        monitors = screens.len(),
        "window was off screen; moved it back"
    );
    if now.w != was.w || now.h != was.h {
        let _ = window.set_size(tauri::PhysicalSize::new(now.w.max(1), now.h.max(1)));
    }
    let _ = window.set_position(tauri::PhysicalPosition::new(now.x, now.y));
}

/// Window ▸ Reset Window Position: the way out when everything else failed.
pub fn recentre<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    let Ok(size) = window.outer_size() else {
        return;
    };
    let Some(primary) = window
        .primary_monitor()
        .ok()
        .flatten()
        .as_ref()
        .map(Rect::from)
    else {
        return;
    };

    let wanted = Rect {
        x: i32::MIN / 2,
        y: i32::MIN / 2,
        w: i32::try_from(size.width).unwrap_or(i32::MAX),
        h: i32::try_from(size.height).unwrap_or(i32::MAX),
    };
    let now = place(wanted, &[], primary);
    let _ = window.set_size(tauri::PhysicalSize::new(now.w.max(1), now.h.max(1)));
    let _ = window.set_position(tauri::PhysicalPosition::new(now.x, now.y));
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAPTOP: Rect = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1040,
    };
    const SECOND: Rect = Rect {
        x: 1920,
        y: 0,
        w: 2560,
        h: 1400,
    };

    fn window(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn an_ordinary_window_is_worth_checking() {
        assert!(should_settle(false, false, false));
    }

    /// The regression: maximize moves the window, the check called that invalid and moved
    /// it back, Windows maximized again — and the window bounced between monitors.
    #[test]
    fn a_maximized_window_is_not_touched() {
        assert!(!should_settle(true, false, false));
    }

    /// A minimized window sits at about (-32000, -32000); moving it from there is how it
    /// stops coming back from the taskbar.
    #[test]
    fn a_minimized_window_is_not_touched() {
        assert!(!should_settle(false, true, false));
    }

    #[test]
    fn a_fullscreen_window_is_not_touched() {
        assert!(!should_settle(false, false, true));
    }

    #[test]
    fn a_window_in_the_middle_of_the_screen_is_left_alone() {
        let wanted = window(200, 150, 1200, 800);
        assert_eq!(place(wanted, &[LAPTOP], LAPTOP), wanted);
    }

    #[test]
    fn a_window_on_the_second_monitor_stays_there() {
        let wanted = window(2200, 300, 1200, 800);
        assert_eq!(place(wanted, &[LAPTOP, SECOND], LAPTOP), wanted);
    }

    /// The reported case: the title bar ended up above the top of the screen.
    #[test]
    fn a_title_bar_above_the_screen_is_brought_back() {
        let placed = place(window(200, -300, 1200, 800), &[LAPTOP], LAPTOP);
        assert!(placed.y >= LAPTOP.y, "{placed:?}");
        assert!(reachable(placed, &[LAPTOP]));
    }

    /// From the log of the three-monitor machine: every maximized and snapped window was
    /// called off screen because its frame hangs a dozen pixels past the work area, and
    /// each verdict moved it. `from=(-4838, -174, 1820, 1164)` was one of sixty thousand.
    #[test]
    fn a_frame_that_overhangs_the_work_area_is_still_reachable() {
        assert!(reachable(window(200, -12, 1920, 1052), &[LAPTOP]));
    }

    /// The three-monitor machine has monitors at negative coordinates; a window there is
    /// where it belongs, not somewhere to be rescued from.
    #[test]
    fn a_monitor_to_the_left_of_the_primary_one_is_a_monitor_like_any_other() {
        let left = Rect {
            x: -5768,
            y: -363,
            w: 1920,
            h: 1080,
        };
        let wanted = window(-5700, -300, 1200, 800);
        assert_eq!(place(wanted, &[left, LAPTOP], LAPTOP), wanted);
    }

    /// Eight pixels of title bar is a grip; none of it is not.
    #[test]
    fn a_title_bar_entirely_above_the_screen_is_still_out_of_reach() {
        assert!(!reachable(window(200, -40, 1200, 800), &[LAPTOP]));
    }

    #[test]
    fn a_window_saved_on_a_monitor_that_is_gone_comes_home() {
        let placed = place(window(2200, 300, 1200, 800), &[LAPTOP], LAPTOP);
        assert!(reachable(placed, &[LAPTOP]), "{placed:?}");
    }

    #[test]
    fn a_window_off_the_right_edge_comes_back() {
        let placed = place(window(1880, 200, 1200, 800), &[LAPTOP], LAPTOP);
        assert!(reachable(placed, &[LAPTOP]), "{placed:?}");
    }

    /// A sliver of title bar at the very edge is not something anyone can aim at.
    #[test]
    fn forty_pixels_of_title_bar_does_not_count_as_reachable() {
        assert!(!reachable(window(1880, 200, 1200, 800), &[LAPTOP]));
    }

    #[test]
    fn a_window_below_the_taskbar_is_brought_back() {
        let placed = place(window(200, 1035, 1200, 800), &[LAPTOP], LAPTOP);
        assert!(reachable(placed, &[LAPTOP]), "{placed:?}");
    }

    /// Saved at 100% on a 4K screen, restored on a 1080p laptop: the window is bigger
    /// than the screen it has to live on.
    #[test]
    fn a_window_larger_than_the_screen_is_cut_down_to_it() {
        let placed = place(window(0, 0, 3840, 2160), &[LAPTOP], LAPTOP);
        assert!(placed.w <= LAPTOP.w && placed.h <= LAPTOP.h, "{placed:?}");
        assert!(reachable(placed, &[LAPTOP]), "{placed:?}");
    }

    #[test]
    fn centring_puts_it_in_the_middle_and_not_at_the_origin() {
        let placed = place(window(-5000, -5000, 1000, 600), &[LAPTOP], LAPTOP);
        assert_eq!(placed.x, (LAPTOP.w - 1000) / 2);
        assert_eq!(placed.y, (LAPTOP.h - 600) / 2);
    }

    #[test]
    fn a_monitor_standing_on_its_side_is_a_monitor_like_any_other() {
        let portrait = Rect {
            x: -1080,
            y: 0,
            w: 1080,
            h: 1920,
        };
        let wanted = window(-900, 100, 800, 1400);
        assert_eq!(place(wanted, &[portrait, LAPTOP], LAPTOP), wanted);
    }

    #[test]
    fn with_no_monitors_at_all_it_still_returns_something_usable() {
        let placed = place(window(200, 150, 1200, 800), &[], LAPTOP);
        assert!(placed.w > 0 && placed.h > 0);
    }
}
