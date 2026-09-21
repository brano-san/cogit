//! One queue for the whole application. The graph hands it the addresses on screen and
//! forgets about them; a picture arrives later or never (M14 T14.2).

use crate::cache::{Cache, Lookup};
use crate::identity::is_noreply;
use parking_lot::{Condvar, Mutex};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

const DEFAULT_WORKERS: usize = 4;

#[derive(Debug)]
pub enum Fetched {
    Image(Vec<u8>),
    /// The service answered, and there is no picture for this address.
    Missing,
    /// The request itself did not complete. Worth one more try, not a remembered miss.
    Failed,
}

pub trait Source: Send + Sync + 'static {
    fn get(&self, email: &str) -> Fetched;
}

#[derive(Default)]
struct State {
    pending: VecDeque<String>,
    queued: HashSet<String>,
    running: HashSet<String>,
    stopped: bool,
}

struct Inner {
    cache: Arc<Cache>,
    source: Arc<dyn Source>,
    ready: Box<dyn Fn(&str) + Send + Sync>,
    state: Mutex<State>,
    work: Condvar,
    idle: Condvar,
}

pub struct Queue {
    inner: Arc<Inner>,
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
    count: Mutex<usize>,
}

impl std::fmt::Debug for Queue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Queue").finish_non_exhaustive()
    }
}

impl Queue {
    pub fn new<S: Source>(
        cache: Arc<Cache>,
        source: Arc<S>,
        ready: impl Fn(&str) + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                cache,
                source,
                ready: Box::new(ready),
                state: Mutex::new(State::default()),
                work: Condvar::new(),
                idle: Condvar::new(),
            }),
            workers: Mutex::new(Vec::new()),
            count: Mutex::new(DEFAULT_WORKERS),
        }
    }

    pub fn with_workers(self, workers: usize) -> Self {
        *self.count.lock() = workers.max(1);
        self
    }

    /// The addresses visible right now, in the order they should arrive. Anything queued
    /// and not named here is dropped: those rows have scrolled away.
    pub fn request(&self, window: &[String]) {
        {
            let mut state = self.inner.state.lock();
            state.pending.clear();
            state.queued.clear();

            for email in window {
                let trimmed = email.trim();
                if trimmed.is_empty() || state.queued.contains(trimmed) {
                    continue;
                }
                if state.running.contains(trimmed) {
                    state.queued.insert(trimmed.to_string());
                    continue;
                }
                if is_noreply(trimmed) {
                    let _ = self.inner.cache.store_missing(trimmed);
                    continue;
                }
                if self.inner.cache.lookup(trimmed) != Lookup::Unknown {
                    continue;
                }
                state.queued.insert(trimmed.to_string());
                state.pending.push_back(trimmed.to_string());
            }
        }
        self.start();
        self.inner.work.notify_all();
    }

    /// Blocks until the queue is empty. For tests and for shutdown, never for the UI.
    pub fn drain(&self) {
        let mut state = self.inner.state.lock();
        while !state.pending.is_empty() || !state.running.is_empty() {
            self.inner.idle.wait(&mut state);
        }
    }

    fn start(&self) {
        let mut workers = self.workers.lock();
        if !workers.is_empty() {
            return;
        }
        for _ in 0..*self.count.lock() {
            let inner = Arc::clone(&self.inner);
            workers.push(std::thread::spawn(move || work(&inner)));
        }
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        self.inner.state.lock().stopped = true;
        self.inner.work.notify_all();
        for worker in self.workers.lock().drain(..) {
            let _ = worker.join();
        }
    }
}

fn work(inner: &Inner) {
    loop {
        let email = {
            let mut state = inner.state.lock();
            loop {
                if state.stopped {
                    return;
                }
                if let Some(email) = state.pending.pop_front() {
                    state.queued.remove(&email);
                    state.running.insert(email.clone());
                    break email;
                }
                inner.work.wait(&mut state);
            }
        };

        fetch(inner, &email);

        let mut state = inner.state.lock();
        state.running.remove(&email);
        if state.pending.is_empty() && state.running.is_empty() {
            inner.idle.notify_all();
        }
    }
}

/// One retry, then the address is remembered as missing: a service that is down would
/// otherwise be asked again on every repaint for the rest of the session.
fn fetch(inner: &Inner, email: &str) {
    for attempt in 0..2 {
        match inner.source.get(email) {
            Fetched::Image(bytes) => {
                match inner.cache.store(email, &bytes) {
                    Ok(_) => (inner.ready)(email),
                    Err(error) => tracing::warn!(?error, "could not store an avatar"),
                }
                return;
            }
            Fetched::Missing => {
                let _ = inner.cache.store_missing(email);
                return;
            }
            Fetched::Failed if attempt == 1 => {
                let _ = inner.cache.store_missing(email);
                return;
            }
            Fetched::Failed => (),
        }
    }
}
