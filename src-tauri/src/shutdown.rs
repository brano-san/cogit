//! Closing the window must not depend on the webview being well.
//!
//! `CloseRequested` is answered in the page: `wiring.ts` asks whether there is unsaved
//! work and only then lets the window go. That is the right place for the question — and
//! the wrong place for it to be the only answer, because a renderer that is busy, hung or
//! dead never gets round to replying and the close button stops doing anything.
//!
//! Measured before the fix: with the renderer's main thread blocked, `WM_CLOSE` left the
//! app running past twenty seconds with all six WebView2 processes alive (problem 13).
//!
//! So the page is still asked — but it is also asked whether it is *there*, and that
//! question has a deadline.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// How long the page has to show it is alive.
///
/// Not how long it has to decide: once it answers the ping it may take as long as the
/// user needs over a confirmation dialog.
pub const GRACE: Duration = Duration::from_secs(2);

/// Asks the page to prove it is running. Injected rather than imported, so nothing in
/// `frontend/` has to know this exists.
const PING: &str = "window.__TAURI_INTERNALS__.invoke('closing_ping')";

static CLOSING: AtomicBool = AtomicBool::new(false);
static ANSWERED: AtomicBool = AtomicBool::new(false);

/// Arms the watchdog. `false` when one is already running — a second click on the cross.
fn arming() -> bool {
    !CLOSING.swap(true, Ordering::SeqCst)
}

fn stand_down() {
    CLOSING.store(false, Ordering::SeqCst);
}

/// The page answered the ping: it is alive, and whatever it does with the close now is
/// its own business, including refusing it.
pub fn answered() {
    ANSWERED.store(true, Ordering::SeqCst);
}

/// Watches one close request.
///
/// The window is deliberately **not** hidden up front. The page asks about unsaved hook
/// edits and unresolved merges, and hiding the window first would leave that question on
/// an invisible dialog. A close the user then cancels has to leave the app as it was.
pub fn watch(app: &tauri::AppHandle) {
    use tauri::Manager as _;

    if !arming() {
        return;
    }
    ANSWERED.store(false, Ordering::SeqCst);

    if let Some(window) = app.get_webview_window("main")
        && let Err(err) = window.eval(PING)
    {
        tracing::warn!(error = %err, "cannot ask the webview whether it is alive");
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(GRACE).await;

        if ANSWERED.load(Ordering::SeqCst) {
            // Alive. It is either closing itself or asking the user; either way this is
            // no longer a hang, and the decision belongs to the page.
            tracing::debug!("the webview answered the close; leaving it to decide");
            stand_down();
            return;
        }

        tracing::warn!(
            grace_ms = u64::try_from(GRACE.as_millis()).unwrap_or(u64::MAX),
            "the webview did not answer the close in time; exiting anyway"
        );

        // Hidden only now: the app is going, and the last thing the user should see is
        // not a frozen window (problem 13).
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }

        app.exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        CLOSING.store(false, Ordering::SeqCst);
        ANSWERED.store(false, Ordering::SeqCst);
    }

    #[test]
    fn the_watchdog_is_armed_once() {
        reset();
        assert!(arming(), "the first close arms it");
        assert!(!arming(), "a second click must not start a second timer");
        assert!(CLOSING.load(Ordering::SeqCst));
        reset();
    }

    #[test]
    fn standing_down_allows_a_later_close() {
        reset();
        assert!(arming());
        stand_down();
        assert!(!CLOSING.load(Ordering::SeqCst));
        assert!(arming(), "a cancelled close must not block the next one");
        reset();
    }

    #[test]
    fn an_answer_is_remembered() {
        reset();
        assert!(!ANSWERED.load(Ordering::SeqCst));
        answered();
        assert!(ANSWERED.load(Ordering::SeqCst));
        reset();
    }
}
