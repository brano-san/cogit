#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The queue exists so that drawing a row never waits on a socket (M14 T14.2). Every test
//! here injects its own source: the real one is the only part that needs the network.

use avatars::{Cache, Fetched, Lookup, Queue, Source};
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct Recording {
    asked: Mutex<Vec<String>>,
    answer: Box<dyn Fn(&str) -> Fetched + Send + Sync>,
    in_flight: AtomicUsize,
    peak: AtomicUsize,
    hold: Option<Duration>,
}

impl Recording {
    fn new(answer: impl Fn(&str) -> Fetched + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            asked: Mutex::new(Vec::new()),
            answer: Box::new(answer),
            in_flight: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            hold: None,
        })
    }

    fn slow(answer: impl Fn(&str) -> Fetched + Send + Sync + 'static, hold: Duration) -> Arc<Self> {
        let mut source = Self::new(answer);
        Arc::get_mut(&mut source).unwrap().hold = Some(hold);
        source
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().clone()
    }
}

impl Source for Recording {
    fn get(&self, email: &str) -> Fetched {
        self.asked.lock().push(email.to_string());
        let running = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(running, Ordering::SeqCst);
        if let Some(hold) = self.hold {
            std::thread::sleep(hold);
        }
        let answer = (self.answer)(email);
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        answer
    }
}

fn cache(dir: &tempfile::TempDir) -> Arc<Cache> {
    Arc::new(Cache::open(dir.path().to_path_buf()).unwrap())
}

/// Polls rather than sleeps a fixed time: a fixed sleep is either flaky or slow.
fn until(deadline: Duration, done: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    done()
}

#[test]
fn a_found_picture_lands_in_the_cache_and_is_announced() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache(&dir);
    let ready = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&ready);

    let queue = Queue::new(
        Arc::clone(&cache),
        Recording::new(|_| Fetched::Image(b"png".to_vec())),
        move |email: &str| seen.lock().push(email.to_string()),
    );
    queue.request(&["ada@example.com".to_string()]);

    assert!(until(Duration::from_secs(5), || !ready.lock().is_empty()));
    assert_eq!(ready.lock().as_slice(), ["ada@example.com"]);
    assert!(matches!(cache.lookup("ada@example.com"), Lookup::Hit(_)));
}

#[test]
fn a_missing_picture_is_remembered_as_missing() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache(&dir);

    let queue = Queue::new(
        Arc::clone(&cache),
        Recording::new(|_| Fetched::Missing),
        |_: &str| {},
    );
    queue.request(&["nobody@example.com".to_string()]);

    assert!(until(Duration::from_secs(5), || cache
        .lookup("nobody@example.com")
        == Lookup::Missing));
}

#[test]
fn an_address_already_in_the_cache_is_never_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache(&dir);
    cache.store("ada@example.com", b"png").unwrap();

    let source = Recording::new(|_| Fetched::Image(b"other".to_vec()));
    let queue = Queue::new(Arc::clone(&cache), Arc::clone(&source), |_: &str| {});
    queue.request(&["ada@example.com".to_string()]);
    queue.drain();

    assert!(source.asked().is_empty());
}

#[test]
fn a_remembered_miss_is_not_asked_for_again() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache(&dir);
    cache.store_missing("nobody@example.com").unwrap();

    let source = Recording::new(|_| Fetched::Image(b"png".to_vec()));
    let queue = Queue::new(Arc::clone(&cache), Arc::clone(&source), |_: &str| {});
    queue.request(&["nobody@example.com".to_string()]);
    queue.drain();

    assert!(source.asked().is_empty());
}

#[test]
fn the_same_address_twice_is_one_request() {
    let dir = tempfile::tempdir().unwrap();
    let source = Recording::new(|_| Fetched::Image(b"png".to_vec()));
    let queue = Queue::new(cache(&dir), Arc::clone(&source), |_: &str| {});

    queue.request(&["ada@example.com".to_string(), "ada@example.com".to_string()]);
    queue.drain();

    assert_eq!(source.asked().len(), 1);
}

#[test]
fn a_new_window_cancels_the_rows_that_scrolled_away() {
    let dir = tempfile::tempdir().unwrap();
    let source = Recording::slow(
        |_| Fetched::Image(b"png".to_vec()),
        Duration::from_millis(80),
    );
    let queue = Queue::new(cache(&dir), Arc::clone(&source), |_: &str| {}).with_workers(1);

    queue.request(&[
        "first@example.com".to_string(),
        "gone@example.com".to_string(),
        "also-gone@example.com".to_string(),
    ]);
    // The single worker is busy with the first; the rest are still only queued.
    queue.request(&[
        "first@example.com".to_string(),
        "stayed@example.com".to_string(),
    ]);
    queue.drain();

    let asked = source.asked();
    assert!(
        asked.contains(&"stayed@example.com".to_string()),
        "{asked:?}"
    );
    assert!(
        !asked.contains(&"gone@example.com".to_string()),
        "{asked:?}"
    );
    assert!(
        !asked.contains(&"also-gone@example.com".to_string()),
        "{asked:?}"
    );
}

#[test]
fn no_more_than_four_requests_are_in_flight() {
    let dir = tempfile::tempdir().unwrap();
    let source = Recording::slow(|_| Fetched::Missing, Duration::from_millis(40));
    let queue = Queue::new(cache(&dir), Arc::clone(&source), |_: &str| {});

    let window: Vec<String> = (0..12).map(|n| format!("author{n}@example.com")).collect();
    queue.request(&window);
    queue.drain();

    assert!(
        source.peak.load(Ordering::SeqCst) <= 4,
        "{}",
        source.peak.load(Ordering::SeqCst)
    );
}

#[test]
fn a_failure_is_retried_once() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache(&dir);
    let attempts = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&attempts);

    let source = Recording::new(move |_| {
        if counter.fetch_add(1, Ordering::SeqCst) == 0 {
            Fetched::Failed
        } else {
            Fetched::Image(b"png".to_vec())
        }
    });
    let queue = Queue::new(Arc::clone(&cache), source, |_: &str| {});
    queue.request(&["ada@example.com".to_string()]);
    queue.drain();

    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert!(matches!(cache.lookup("ada@example.com"), Lookup::Hit(_)));
}

#[test]
fn a_second_failure_becomes_a_miss_rather_than_an_endless_retry() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache(&dir);
    let source = Recording::new(|_| Fetched::Failed);

    let queue = Queue::new(Arc::clone(&cache), Arc::clone(&source), |_: &str| {});
    queue.request(&["ada@example.com".to_string()]);
    queue.drain();

    assert_eq!(source.asked().len(), 2);
    assert_eq!(cache.lookup("ada@example.com"), Lookup::Missing);
}

#[test]
fn asking_for_a_window_does_not_block_the_caller() {
    let dir = tempfile::tempdir().unwrap();
    let source = Recording::slow(|_| Fetched::Missing, Duration::from_millis(200));
    let queue = Queue::new(cache(&dir), source, |_: &str| {});

    let window: Vec<String> = (0..40).map(|n| format!("author{n}@example.com")).collect();
    let start = Instant::now();
    queue.request(&window);

    assert!(
        start.elapsed() < Duration::from_millis(100),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn a_noreply_address_is_never_sent_anywhere() {
    let dir = tempfile::tempdir().unwrap();
    let source = Recording::new(|_| Fetched::Image(b"png".to_vec()));
    let queue = Queue::new(cache(&dir), Arc::clone(&source), |_: &str| {});

    queue.request(&["12345+octocat@users.noreply.github.com".to_string()]);
    queue.drain();

    assert!(source.asked().is_empty());
}

#[test]
fn a_picture_answer_is_an_image() {
    assert!(matches!(
        avatars::outcome_for(200, b"png".to_vec()),
        Fetched::Image(_)
    ));
}

#[test]
fn a_404_is_the_service_saying_there_is_none() {
    assert!(matches!(
        avatars::outcome_for(404, Vec::new()),
        Fetched::Missing
    ));
}

#[test]
fn a_server_error_is_worth_one_more_try() {
    assert!(matches!(
        avatars::outcome_for(500, Vec::new()),
        Fetched::Failed
    ));
}

#[test]
fn being_rate_limited_is_not_a_missing_picture() {
    // Remembering 429 as a miss would blank every author for the next seven days.
    assert!(matches!(
        avatars::outcome_for(429, Vec::new()),
        Fetched::Failed
    ));
}

#[test]
fn an_empty_two_hundred_is_not_a_picture() {
    assert!(matches!(
        avatars::outcome_for(200, Vec::new()),
        Fetched::Missing
    ));
}

// `request` runs on every scroll of the graph and promises to touch no file; for a noreply
// address it wrote the cache index each time, most authors of a GitHub repository among them.
#[test]
fn a_noreply_address_already_known_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let source = Recording::new(|_| Fetched::Image(b"png".to_vec()));
    let queue = Queue::new(cache(&dir), Arc::clone(&source), |_: &str| {});
    let noreply = ["12345+octocat@users.noreply.github.com".to_string()];
    queue.request(&noreply);
    queue.drain();
    let index = dir.path().join("index.json");
    let _ = std::fs::remove_file(&index);

    queue.request(&noreply);
    queue.drain();

    assert!(!index.exists(), "the index was written again");
}
