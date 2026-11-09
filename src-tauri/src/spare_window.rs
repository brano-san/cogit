use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::Manager as _;

use crate::child_window::{MAIN, Shape, Submenu};

const KIND: &str = "spare";
const PAGE: &str = "spare.html";
pub const FIRST_AFTER: Duration = Duration::from_secs(3);

static BUILDING: AtomicBool = AtomicBool::new(false);
static WAITING: parking_lot::Mutex<Option<Waiting>> = parking_lot::Mutex::new(None);

struct Waiting {
    label: String,
    answer: tokio::sync::oneshot::Sender<String>,
}

pub struct Spare<R: tauri::Runtime> {
    window: tauri::WebviewWindow<R>,
    answer: tokio::sync::oneshot::Sender<String>,
}

pub fn prepare_later<R: tauri::Runtime>(app: &tauri::AppHandle<R>, after: Duration) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(after);
        prepare(&app);
    });
}

pub fn prepare<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if BUILDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if app.get_webview_window(MAIN).is_some()
            && let Err(err) = build(&app)
        {
            tracing::warn!(error = %err, "cannot prepare a spare window");
            BUILDING.store(false, Ordering::Release);
        }
    });
}

fn build<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let window = crate::child_window::hidden_builder(
        app,
        crate::child_window::next_label(KIND),
        PAGE.to_owned(),
    )
    .build()?;
    window.remove_menu()?;
    #[cfg(windows)]
    crate::webview2::harden(&window);
    Ok(())
}

pub async fn wait<R: tauri::Runtime>(window: tauri::WebviewWindow<R>) -> Option<String> {
    let (answer, asked) = tokio::sync::oneshot::channel();
    *WAITING.lock() = Some(Waiting {
        label: window.label().to_owned(),
        answer,
    });
    BUILDING.store(false, Ordering::Release);
    asked.await.ok()
}

pub fn claim<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<Spare<R>> {
    let waiting = WAITING.lock().take()?;
    let window = app.get_webview_window(&waiting.label)?;
    Some(Spare {
        window,
        answer: waiting.answer,
    })
}

#[must_use]
pub fn page_address(url: &str) -> String {
    format!("/{}", url.trim_start_matches('/'))
}

impl<R: tauri::Runtime> Spare<R> {
    pub fn label(&self) -> &str {
        self.window.label()
    }

    pub fn become_window(
        self,
        app: &tauri::AppHandle<R>,
        url: String,
        title: &str,
        shape: &Shape,
        menu: &'static [Submenu],
    ) -> tauri::Result<()> {
        let Self { window, answer } = self;
        window.set_title(title)?;
        window.set_theme(Some(crate::native_theme::current()))?;
        window.set_background_color(Some(crate::native_theme::background()))?;
        if !menu.is_empty() {
            window.set_menu(crate::child_window::build_menu(app, window.label(), menu)?)?;
        }
        window.set_min_size(Some(tauri::LogicalSize::new(
            shape.min_width,
            shape.min_height,
        )))?;
        window.set_size(tauri::LogicalSize::new(shape.width, shape.height))?;
        if let Some(main) = app.get_webview_window(MAIN) {
            crate::window_place::centre_on_monitor_of(&window, &main);
        }
        #[cfg(windows)]
        crate::webview2::install_child_accelerators(
            &window,
            crate::child_window::accelerator_table(window.label(), menu),
        );
        if answer.send(page_address(&url)).is_err() {
            tracing::warn!(window = window.label(), "the spare window stopped waiting");
        }
        window.show()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_child_address_becomes_a_page_path() {
        assert_eq!(page_address("commit.html?repo=1"), "/commit.html?repo=1");
        assert_eq!(page_address("/blame.html"), "/blame.html");
    }
}
