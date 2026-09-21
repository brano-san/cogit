//! The application's one avatar service. It is absent until the user turns avatars on,
//! and while it is absent nothing is fetched and no cache directory exists (M14 T14.3).

use crate::AppEvent;
use avatars::{Cache, Lookup, Queue, Source, data_url, fallback};
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

    /// The window the graph is showing. Everything not named here is dropped from the
    /// queue, so scrolling past a thousand rows does not cost a thousand requests.
    pub fn rows(&self, authors: &[Author]) -> Vec<AvatarRow> {
        let wanted: Vec<String> = authors
            .iter()
            .map(|a| a.email.clone())
            .filter(|e| !e.trim().is_empty())
            .collect();
        self.queue.request(&wanted);

        authors.iter().map(|a| self.row(a)).collect()
    }

    fn row(&self, author: &Author) -> AvatarRow {
        let look = fallback(&author.name, &author.email);
        let image = match self.cache.lookup(&author.email) {
            Lookup::Hit(path) => std::fs::read(path).ok().map(|bytes| data_url(&bytes)),
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
