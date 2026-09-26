#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The cache is what keeps the promise that a second run makes no network requests
//! (M14 T14.1). Its clock is injectable so the TTLs can be tested without waiting a month.

use avatars::{Cache, Lookup};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DAY: u64 = 24 * 60 * 60;
const PNG: &[u8] = b"not really a png, but bytes are bytes";

struct Clock(Arc<AtomicU64>);

impl Clock {
    fn new() -> (Self, Arc<AtomicU64>) {
        let shared = Arc::new(AtomicU64::new(1_000 * DAY));
        (Self(Arc::clone(&shared)), shared)
    }
}

fn cache(dir: &tempfile::TempDir) -> (Cache, Arc<AtomicU64>) {
    let (clock, handle) = Clock::new();
    let ticks = Arc::clone(&clock.0);
    let cache = Cache::open(dir.path().to_path_buf())
        .unwrap()
        .with_clock(move || ticks.load(Ordering::Relaxed));
    (cache, handle)
}

#[test]
fn a_stored_picture_comes_back_by_the_same_address() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, _) = cache(&dir);

    cache.store("ada@example.com", PNG).unwrap();

    match cache.lookup("ada@example.com") {
        Lookup::Hit(path) => assert_eq!(std::fs::read(path).unwrap(), PNG),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_address_never_seen_is_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, _) = cache(&dir);
    assert_eq!(cache.lookup("nobody@example.com"), Lookup::Unknown);
}

#[test]
fn a_remembered_miss_is_not_asked_for_again() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, _) = cache(&dir);

    cache.store_missing("nobody@example.com").unwrap();

    assert_eq!(cache.lookup("nobody@example.com"), Lookup::Missing);
}

#[test]
fn case_and_spacing_reach_the_same_entry() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, _) = cache(&dir);

    cache.store("  Ada@Example.COM ", PNG).unwrap();

    assert!(matches!(cache.lookup("ada@example.com"), Lookup::Hit(_)));
}

#[test]
fn the_file_sits_flat_in_the_directory_under_the_hash() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, _) = cache(&dir);

    let path = cache.store("ada@example.com", PNG).unwrap();

    let expected = dir
        .path()
        .join(format!("{}.png", avatars::email_hash("ada@example.com")));
    assert_eq!(path, expected);
}

#[test]
fn a_picture_older_than_thirty_days_is_fetched_again() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, clock) = cache(&dir);

    cache.store("ada@example.com", PNG).unwrap();
    clock.fetch_add(31 * DAY, Ordering::Relaxed);

    assert_eq!(cache.lookup("ada@example.com"), Lookup::Unknown);
}

#[test]
fn a_picture_younger_than_thirty_days_is_still_served() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, clock) = cache(&dir);

    cache.store("ada@example.com", PNG).unwrap();
    clock.fetch_add(29 * DAY, Ordering::Relaxed);

    assert!(matches!(cache.lookup("ada@example.com"), Lookup::Hit(_)));
}

#[test]
fn a_miss_is_retried_after_seven_days_not_thirty() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, clock) = cache(&dir);

    cache.store_missing("nobody@example.com").unwrap();
    clock.fetch_add(8 * DAY, Ordering::Relaxed);

    assert_eq!(cache.lookup("nobody@example.com"), Lookup::Unknown);
}

#[test]
fn a_fresh_miss_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, clock) = cache(&dir);

    cache.store_missing("nobody@example.com").unwrap();
    clock.fetch_add(6 * DAY, Ordering::Relaxed);

    assert_eq!(cache.lookup("nobody@example.com"), Lookup::Missing);
}

#[test]
fn the_second_run_reads_what_the_first_one_wrote() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (cache, _) = cache(&dir);
        cache.store("ada@example.com", PNG).unwrap();
        cache.store_missing("nobody@example.com").unwrap();
    }

    let (reopened, _) = cache(&dir);
    assert!(matches!(reopened.lookup("ada@example.com"), Lookup::Hit(_)));
    assert_eq!(reopened.lookup("nobody@example.com"), Lookup::Missing);
}

#[test]
fn a_corrupt_index_costs_the_cache_not_the_run() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.json"), "{ this is not json").unwrap();

    let (cache, _) = cache(&dir);
    assert_eq!(cache.lookup("ada@example.com"), Lookup::Unknown);
}

#[test]
fn the_directory_stays_under_its_limit() {
    let dir = tempfile::tempdir().unwrap();
    let (clock, _handle) = Clock::new();
    let ticks = Arc::clone(&clock.0);
    let cache = Cache::open(dir.path().to_path_buf())
        .unwrap()
        .with_clock(move || ticks.load(Ordering::Relaxed))
        .with_limit(PNG.len() as u64 * 2);

    for n in 0..5 {
        cache.store(&format!("author{n}@example.com"), PNG).unwrap();
    }

    let png_files = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .count();
    assert!(png_files <= 2, "{png_files} files kept");
}

#[test]
fn eviction_drops_the_least_recently_used_author() {
    let dir = tempfile::tempdir().unwrap();
    let (clock, handle) = Clock::new();
    let ticks = Arc::clone(&clock.0);
    let cache = Cache::open(dir.path().to_path_buf())
        .unwrap()
        .with_clock(move || ticks.load(Ordering::Relaxed))
        .with_limit(PNG.len() as u64 * 2);

    cache.store("old@example.com", PNG).unwrap();
    handle.fetch_add(60, Ordering::Relaxed);
    cache.store("new@example.com", PNG).unwrap();
    handle.fetch_add(60, Ordering::Relaxed);
    // Reading counts as using it, so the older file is the one that goes.
    assert!(matches!(cache.lookup("old@example.com"), Lookup::Hit(_)));

    handle.fetch_add(60, Ordering::Relaxed);
    cache.store("third@example.com", PNG).unwrap();

    assert!(matches!(cache.lookup("old@example.com"), Lookup::Hit(_)));
    assert_eq!(cache.lookup("new@example.com"), Lookup::Unknown);
}

#[test]
fn an_evicted_entry_leaves_no_row_behind_in_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let (clock, handle) = Clock::new();
    let ticks = Arc::clone(&clock.0);
    let cache = Cache::open(dir.path().to_path_buf())
        .unwrap()
        .with_clock(move || ticks.load(Ordering::Relaxed))
        .with_limit(PNG.len() as u64);

    cache.store("first@example.com", PNG).unwrap();
    handle.fetch_add(60, Ordering::Relaxed);
    cache.store("second@example.com", PNG).unwrap();

    // Unknown, not Missing: an evicted picture may well still exist upstream.
    assert_eq!(cache.lookup("first@example.com"), Lookup::Unknown);
}

#[test]
fn a_remembered_miss_costs_no_disk_space() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, _) = cache(&dir);

    cache.store_missing("nobody@example.com").unwrap();

    let png_files = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .count();
    assert_eq!(png_files, 0);
}

#[test]
fn reading_a_cached_picture_does_not_rewrite_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, _) = cache(&dir);
    cache.store("ada@example.com", PNG).unwrap();
    cache.flush();

    let index = dir.path().join("index.json");
    let before = std::fs::metadata(&index).unwrap().len();
    let stamp = std::fs::read(&index).unwrap();

    // A window of fifty rows looks up fifty times per repaint; each one must be free.
    for _ in 0..50 {
        assert!(matches!(cache.lookup("ada@example.com"), Lookup::Hit(_)));
    }

    assert_eq!(std::fs::metadata(&index).unwrap().len(), before);
    assert_eq!(std::fs::read(&index).unwrap(), stamp);
}

#[test]
fn the_use_times_reach_the_disk_when_the_cache_is_flushed() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, clock) = cache(&dir);
    cache.store("ada@example.com", PNG).unwrap();
    cache.flush();
    let before = std::fs::read(dir.path().join("index.json")).unwrap();

    clock.fetch_add(60, Ordering::Relaxed);
    let _ = cache.lookup("ada@example.com");
    cache.flush();

    assert_ne!(
        std::fs::read(dir.path().join("index.json")).unwrap(),
        before
    );
}

#[test]
fn a_stale_entry_is_dropped_from_the_index_on_disk_at_once() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (first, clock) = cache(&dir);
        first.store("ada@example.com", PNG).unwrap();
        clock.fetch_add(31 * DAY, Ordering::Relaxed);
        assert_eq!(first.lookup("ada@example.com"), Lookup::Unknown);
    }

    // Eviction removed the file, so the index must not keep pointing at it.
    let (reopened, _) = cache(&dir);
    assert_eq!(reopened.lookup("ada@example.com"), Lookup::Unknown);
}

// Four fetch threads each rewrote the whole index after every picture, with no lock around
// the write: an older snapshot landing last dropped newer entries, and a shorter one left a
// tail behind that made the next run start the cache over.
#[test]
fn pictures_stored_from_four_threads_all_reach_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let emails: Vec<Vec<String>> = (0..4)
        .map(|thread| {
            (0..500)
                .map(|n| format!("author{thread}-{n}@example.com"))
                .collect()
        })
        .collect();
    {
        let (cache, _) = cache(&dir);
        let cache = Arc::new(cache);
        let workers: Vec<_> = emails
            .iter()
            .cloned()
            .map(|mine| {
                let cache = Arc::clone(&cache);
                std::thread::spawn(move || {
                    for email in mine {
                        cache.store(&email, PNG).unwrap();
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
    }

    let (reopened, _) = cache(&dir);
    let lost = emails
        .iter()
        .flatten()
        .filter(|email| !matches!(reopened.lookup(email), Lookup::Hit(_)))
        .count();
    assert_eq!(lost, 0, "of 2000 pictures stored");
}

// Every picture rewrote the whole index: three thousand authors came to 0.6 GB of writes.
#[test]
fn a_stored_picture_reaches_the_index_when_the_cache_is_flushed() {
    let dir = tempfile::tempdir().unwrap();
    let (cache, _) = cache(&dir);
    cache.store("ada@example.com", PNG).unwrap();
    cache.flush();
    let index = dir.path().join("index.json");
    let before = std::fs::read(&index).unwrap();

    cache.store("grace@example.com", PNG).unwrap();
    cache.store_missing("nobody@example.com").unwrap();
    assert_eq!(
        std::fs::read(&index).unwrap(),
        before,
        "not once per picture"
    );

    cache.flush();
    let (reopened, _) = self::cache(&dir);
    assert!(matches!(
        reopened.lookup("grace@example.com"),
        Lookup::Hit(_)
    ));
    assert_eq!(reopened.lookup("nobody@example.com"), Lookup::Missing);
}
