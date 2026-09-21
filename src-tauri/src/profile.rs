//! One grep-able line per operation, on its own tracing target so a day of work can be
//! read back without the rest of the log: `grep 'cogit::profile' cogit.log`.

use git_engine::phases::{PhaseTimer, bottleneck};
use std::time::Duration;

/// Below this an entry says nothing a human would act on, and a scroll produces hundreds.
const FLOOR: Duration = Duration::from_millis(5);

/// Half a second is where a click stops feeling answered. Anything past it earns a warning
/// of its own, so "the interface hung" has something to point at.
const SLOW: Duration = Duration::from_millis(500);

fn millis(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

pub fn call(label: &str, elapsed: Duration, ok: bool) {
    // Every command, however fast. The profile stream below has a floor so that scrolling
    // cannot bury the file, but diagnosing a freeze needs the calls that returned quickly
    // just as much as the one that did not. Default target, so `Log level` governs it.
    tracing::debug!(op = label, ms = millis(elapsed), ok, "ipc");

    if elapsed >= SLOW {
        tracing::warn!(op = label, ms = millis(elapsed), ok, "slow ipc command");
    }

    if elapsed < FLOOR && ok {
        return;
    }
    tracing::info!(target: "cogit::profile", kind = "ipc", op = label, ms = millis(elapsed), ok);
}

/// Reported by the frontend: the interval between a click and the screen showing its result.
pub fn ui(label: &str, elapsed_ms: u64, detail: &str) {
    tracing::info!(target: "cogit::profile", kind = "ui", op = label, ms = elapsed_ms, detail);
}

/// A network operation, split into the phases `git --progress` announces, so a slow pull
/// can be blamed on the right party instead of on the client by default.
pub fn network(op: &str, remote: &str, timer: PhaseTimer, ok: bool) {
    let timings = timer.finish();
    tracing::info!(
        target: "cogit::profile",
        kind = "network",
        op,
        remote,
        ok,
        ms = millis(timings.total),
        objects = timings.objects.unwrap_or_default(),
        bytes = timings.bytes.unwrap_or_default(),
        kib_per_s = timings.transfer_rate().unwrap_or_default() / 1024.0,
        phases = %timings.summary(),
        slowest = bottleneck(&timings),
    );
}

/// What the renderer reports every ten seconds in a debug build.
///
/// A heap figure on its own cannot tell a leak from a large repository, so the counters
/// that separate the three diagnoses — growth with time, with actions, or in one jump —
/// travel with it.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RendererMemory {
    /// KiB, not bytes: specta forbids 64-bit integers over IPC, and 32 bits of KiB is
    /// four terabytes — past anything a renderer can hold.
    pub used_heap_kib: u32,
    pub total_heap_kib: u32,
    pub limit_kib: u32,
    pub dom_nodes: u32,
    pub listeners: u32,
    /// Sorted by key, so two samples an hour apart diff line by line.
    pub caches: std::collections::BTreeMap<String, u32>,
}

/// One `kind=mem` line per sample from the webview.
pub fn renderer(sample: &RendererMemory) {
    tracing::info!(
        target: "cogit::profile",
        kind = "mem",
        used_kib = sample.used_heap_kib,
        total_kib = sample.total_heap_kib,
        limit_kib = sample.limit_kib,
        dom = sample.dom_nodes,
        listeners = sample.listeners,
        caches = %counters(&sample.caches),
    );
}

fn counters(caches: &std::collections::BTreeMap<String, u32>) -> String {
    caches
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One `kind=procmem` line per sample from the host, which outlives the renderer.
pub fn processes(totals: &crate::webview_memory::Totals) {
    tracing::info!(
        target: "cogit::profile",
        kind = "procmem",
        processes = totals.count,
        rss_kib = totals.rss_kib,
        largest_kib = totals.largest_kib,
    );
}
