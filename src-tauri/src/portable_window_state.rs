//! The window geometry of a portable build.
//!
//! `tauri-plugin-window-state` keeps its file in the system's config folder and creates
//! that folder when it saves, which a portable build must not touch. The file keeps the
//! plugin's format, so the same settings folder works with either build. `window_place`
//! still checks the restored position afterwards.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow, WindowEvent};

pub const FILE: &str = ".window-state.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    prev_x: i32,
    prev_y: i32,
    maximized: bool,
    visible: bool,
    decorated: bool,
    fullscreen: bool,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            x: 0,
            y: 0,
            prev_x: 0,
            prev_y: 0,
            maximized: false,
            visible: true,
            decorated: true,
            fullscreen: false,
        }
    }
}

impl Saved {
    fn has_geometry(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// The geometry of the normal (not maximized, not minimized) window.
    fn moved(&mut self, x: i32, y: i32) {
        self.prev_x = self.x;
        self.prev_y = self.y;
        self.x = x;
        self.y = y;
    }

    /// A maximized window is saved with the corner it was maximized at, which is not where
    /// "Restore" should put it: the position before that move is (the plugin does the same).
    fn restore_position(&self) -> (i32, i32) {
        if self.maximized {
            (self.prev_x, self.prev_y)
        } else {
            (self.x, self.y)
        }
    }

    fn resized(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.width = width;
            self.height = height;
        }
    }
}

pub fn read(path: &Path) -> HashMap<String, Saved> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
            tracing::warn!(error = ?err, path = %path.display(), context = "unreadable window state; starting from the default");
            HashMap::new()
        }),
        Err(_) => HashMap::new(),
    }
}

pub fn write(path: &Path, states: &HashMap<String, Saved>) -> std::io::Result<()> {
    let text = serde_json::to_vec_pretty(states).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

fn restore(window: &WebviewWindow, saved: &Saved) -> tauri::Result<()> {
    if !saved.has_geometry() {
        return Ok(());
    }
    let (x, y) = saved.restore_position();
    window.set_position(PhysicalPosition::new(x, y))?;
    window.set_size(PhysicalSize::new(saved.width, saved.height))?;
    if saved.maximized {
        window.maximize()?;
    }
    window.set_fullscreen(saved.fullscreen)
}

/// Restores the window now and keeps `path` up to date until it closes.
pub fn install(window: &WebviewWindow, path: PathBuf) {
    let label = window.label().to_owned();
    let states = Arc::new(Mutex::new(read(&path)));
    let restoring = Arc::new(AtomicBool::new(true));

    let saved = states
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(label.clone())
        .or_default()
        .clone();
    if let Err(err) = restore(window, &saved) {
        tracing::warn!(error = ?err, context = "cannot restore the window state");
    }
    restoring.store(false, Ordering::SeqCst);

    let tracked = window.clone();
    window.on_window_event(move |event| {
        if restoring.load(Ordering::SeqCst) {
            return;
        }
        let minimized = tracked.is_minimized().unwrap_or_default();
        let normal = !minimized && !tracked.is_maximized().unwrap_or_default();
        let mut states = states.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(state) = states.get_mut(&label) else {
            return;
        };
        match event {
            // Not `normal`: tao reports the move that maximizes before it sets the flag, so the
            // position before it must be shifted into `prev_*` (see `restore_position`).
            WindowEvent::Moved(position) if !minimized => state.moved(position.x, position.y),
            WindowEvent::Resized(size) if normal => state.resized(size.width, size.height),
            WindowEvent::CloseRequested { .. } => {
                state.maximized = tracked.is_maximized().unwrap_or_default();
                state.fullscreen = tracked.is_fullscreen().unwrap_or_default();
                if normal {
                    if let Ok(size) = tracked.inner_size() {
                        state.resized(size.width, size.height);
                    }
                    if let Ok(at) = tracked.outer_position() {
                        state.moved(at.x, at.y);
                    }
                }
                if let Err(err) = write(&path, &states) {
                    tracing::error!(error = ?err, path = %path.display(), context = "cannot save the window state");
                }
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn a_file_of_the_plugin_reads_back() {
        let text = r#"{"main":{"width":1600,"height":1000,"x":10,"y":20,"prev_x":0,"prev_y":0,
            "maximized":true,"visible":true,"decorated":true,"fullscreen":false}}"#;
        let states: HashMap<String, Saved> = serde_json::from_str(text).unwrap();
        let main = &states["main"];
        assert!(main.has_geometry() && main.maximized);
        assert_eq!(
            (main.x, main.y, main.width, main.height),
            (10, 20, 1600, 1000)
        );
    }

    #[test]
    fn what_is_written_has_every_field_of_the_plugins_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        let mut states = HashMap::new();
        let mut saved = Saved::default();
        saved.resized(800, 600);
        saved.moved(5, 6);
        states.insert("main".to_owned(), saved.clone());
        write(&path, &states).unwrap();

        assert_eq!(read(&path)["main"], saved);
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["main"].as_object().unwrap().len(), 10);
    }

    #[test]
    fn a_missing_or_broken_file_is_an_empty_state() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(&dir.path().join(FILE)).is_empty());
        std::fs::write(dir.path().join(FILE), "not json").unwrap();
        assert!(read(&dir.path().join(FILE)).is_empty());
    }

    #[test]
    fn a_window_closed_maximized_comes_back_where_it_was_before() {
        let mut saved = Saved::default();
        saved.resized(800, 600);
        saved.moved(100, 100);
        saved.moved(-8, -8); // the move that maximizes
        saved.maximized = true;
        assert_eq!(saved.restore_position(), (100, 100));
    }

    #[test]
    fn a_normal_window_comes_back_at_its_last_position() {
        let mut saved = Saved::default();
        saved.moved(100, 100);
        saved.moved(30, 40);
        assert_eq!(saved.restore_position(), (30, 40));
    }

    #[test]
    fn an_empty_size_is_never_saved() {
        let mut saved = Saved::default();
        saved.resized(0, 0);
        assert!(!saved.has_geometry());
    }
}
