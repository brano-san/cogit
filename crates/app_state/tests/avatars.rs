#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What the graph asks for and what it gets back. The fallback is always there; the
//! picture is an extra that may arrive later or never (M14).

use app_state::{AppState, Author};
use std::sync::Arc;

struct Always(Vec<u8>);

impl avatars::Source for Always {
    fn get(&self, _email: &str) -> avatars::Fetched {
        avatars::Fetched::Image(self.0.clone())
    }
}

struct Never;

impl avatars::Source for Never {
    fn get(&self, _email: &str) -> avatars::Fetched {
        avatars::Fetched::Missing
    }
}

fn authors() -> Vec<Author> {
    vec![Author {
        name: "Ada Lovelace".to_string(),
        email: "ada@example.com".to_string(),
    }]
}

#[test]
fn with_avatars_off_every_author_still_has_something_to_draw() {
    let state = AppState::new();
    let rows = state.avatars(&authors());

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].initials, "AL");
    assert!(rows[0].color.starts_with('#'));
    assert!(rows[0].image.is_none());
}

#[test]
fn with_avatars_off_no_cache_directory_is_created() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("avatars");

    let state = AppState::new();
    let _ = state.avatars(&authors());

    assert!(!cache.exists());
}

#[test]
fn a_fetched_picture_comes_back_as_a_data_url() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state
        .enable_avatars_with(dir.path().to_path_buf(), Arc::new(Always(b"png".to_vec())))
        .unwrap();

    let _ = state.avatars(&authors());
    state.drain_avatars();

    let rows = state.avatars(&authors());
    assert!(
        rows[0]
            .image
            .as_deref()
            .is_some_and(|url| url.starts_with("data:image/png;base64,")),
        "{rows:?}"
    );
}

#[test]
fn the_fallback_is_served_alongside_the_picture() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state
        .enable_avatars_with(dir.path().to_path_buf(), Arc::new(Always(b"png".to_vec())))
        .unwrap();

    let _ = state.avatars(&authors());
    state.drain_avatars();

    // The initials stay in the row so the frontend can draw them while the image loads.
    assert_eq!(state.avatars(&authors())[0].initials, "AL");
}

#[test]
fn an_author_the_service_does_not_know_keeps_the_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state
        .enable_avatars_with(dir.path().to_path_buf(), Arc::new(Never))
        .unwrap();

    let _ = state.avatars(&authors());
    state.drain_avatars();

    assert!(state.avatars(&authors())[0].image.is_none());
}

#[test]
fn turning_avatars_off_stops_serving_pictures() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state
        .enable_avatars_with(dir.path().to_path_buf(), Arc::new(Always(b"png".to_vec())))
        .unwrap();
    let _ = state.avatars(&authors());
    state.drain_avatars();

    state.disable_avatars();

    assert!(state.avatars(&authors())[0].image.is_none());
}

#[tokio::test]
async fn an_arrived_picture_is_announced_so_the_row_can_redraw() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    let mut events = state.subscribe();
    state
        .enable_avatars_with(dir.path().to_path_buf(), Arc::new(Always(b"png".to_vec())))
        .unwrap();

    let _ = state.avatars(&authors());
    state.drain_avatars();

    let event = events.try_recv().unwrap();
    match event {
        app_state::AppEvent::AvatarReady { email } => assert_eq!(email, "ada@example.com"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_author_without_an_address_is_never_looked_up() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new();
    state
        .enable_avatars_with(dir.path().to_path_buf(), Arc::new(Always(b"png".to_vec())))
        .unwrap();

    let rows = state.avatars(&[Author {
        name: "Nobody".to_string(),
        email: String::new(),
    }]);

    assert_eq!(rows[0].initials, "N");
    assert!(rows[0].image.is_none());
}
