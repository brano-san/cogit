//! The application's one avatar service. It is absent until the user turns avatars on,
//! and while it is absent nothing is fetched and no cache directory exists (M14 T14.3).

use crate::AppEvent;
use avatars::{Cache, Lookup, Queue, Source, fallback};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AvatarRow {
    pub email: String,
    pub initials: String,
    pub color: String,
    /// A `data:` URL, so the webview needs no access to the cache directory.
    pub image: Option<String>,
}

pub struct Avatars {
    cache: Arc<Cache>,
    queue: Queue,
}

impl std::fmt::Debug for Avatars {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Avatars").finish_non_exhaustive()
    }
}

impl Avatars {
    pub fn new<S: Source>(
        dir: PathBuf,
        source: Arc<S>,
        events: broadcast::Sender<AppEvent>,
    ) -> Result<Self, avatars::CacheError> {
        let cache = Arc::new(Cache::open(dir)?);
        let queue = Queue::new(Arc::clone(&cache), source, move |email: &str| {
            let _ = events.send(AppEvent::AvatarReady {
                email: email.to_string(),
            });
        });
        Ok(Self { cache, queue })
    }

    /// The addresses on screen. Everything not named here is dropped from the queue,
    /// so scrolling past a thousand rows does not cost a thousand requests. Cheap by
    /// design: the caller sends it on every scroll, and it touches no file.
    pub fn window(&self, emails: &[String]) {
        let wanted: Vec<String> = emails
            .iter()
            .filter(|e| !e.trim().is_empty())
            .cloned()
            .collect();
        self.queue.request(&wanted);
    }

    /// Reads the pictures for these authors. Each one costs a file read and a base64
    /// encode, so the caller asks only for what it does not already hold.
    pub fn rows(&self, authors: &[Author]) -> Vec<AvatarRow> {
        authors.iter().map(|a| self.row(a)).collect()
    }

    fn row(&self, author: &Author) -> AvatarRow {
        let look = fallback(&author.name, &author.email);
        let image = match self.cache.lookup(&author.email) {
            Lookup::Hit(path) => std::fs::read(path)
                .ok()
                .map(|bytes| diff_engine::data_url("image/png", &bytes)),
            _ => None,
        };
        AvatarRow {
            email: author.email.clone(),
            initials: look.initials,
            color: look.color,
            image,
        }
    }

    pub fn drain(&self) {
        self.queue.drain();
    }
}

/// Every author is drawable without a service; this is the whole of the `off` path.
pub fn rows_without_pictures(authors: &[Author]) -> Vec<AvatarRow> {
    authors
        .iter()
        .map(|author| {
            let look = fallback(&author.name, &author.email);
            AvatarRow {
                email: author.email.clone(),
                initials: look.initials,
                color: look.color,
                image: None,
            }
        })
        .collect()
}
