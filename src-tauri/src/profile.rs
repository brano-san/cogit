//! One grep-able line per operation, on its own tracing target so a day of work can be
//! read back without the rest of the log: `grep 'cogit::profile' cogit.log`.

use git_engine::phases::{PhaseTimer, bottleneck};
use std::time::Duration;

/// Below this an entry says nothing a human would act on, and a scroll produces hundreds.
const FLOOR: Duration = Duration::from_millis(5);

fn millis(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

pub fn call(label: &str, elapsed: Duration, ok: bool) {
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
