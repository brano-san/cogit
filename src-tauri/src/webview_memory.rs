//! Working set of the WebView2 processes that belong to this app.
//!
//! The renderer's own probe dies with the renderer. This one runs in the host process,
//! which survives an `Out of Memory`, so the last figures before the crash are still in
//! the log when the user brings it in (doc/14-profiling.md, `kind=procmem`).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// The browser process, the renderer, the GPU process and every utility process all run
/// under this one name.
const WEBVIEW_EXE: &str = "msedgewebview2.exe";

/// The cadence of the renderer probe, so the two series line up in the log.
pub const SAMPLE_EVERY: Duration = Duration::from_secs(10);

/// One process, reduced to what the sum needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub rss_kib: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    pub count: usize,
    pub rss_kib: u64,
    /// The renderer is normally the largest of them, so this is the figure that moves
    /// first when it is the one leaking.
    pub largest_kib: u64,
}

/// Sums the WebView2 processes descended from `root`.
///
/// Descendants rather than children: WebView2 puts the renderer and the GPU process below
/// its own browser process, so counting only direct children would miss the very process
/// that dies. The walk starts at our own pid because other applications on the machine run
/// WebView2 too, and matching on the name alone would add their memory to ours.
#[must_use]
pub fn totals(processes: &[Process], root: u32) -> Totals {
    let mut children: HashMap<u32, Vec<&Process>> = HashMap::new();
    for process in processes {
        if let Some(parent) = process.parent {
            children.entry(parent).or_default().push(process);
        }
    }

    let mut summed = Totals::default();
    let mut seen: HashSet<u32> = HashSet::from([root]);
    let mut queue = vec![root];

    // Windows reuses pids, which can point a parent back into the walk. `seen` is what
    // keeps that finite rather than hanging the sampler.
    while let Some(pid) = queue.pop() {
        for child in children.get(&pid).into_iter().flatten() {
            if !seen.insert(child.pid) {
                continue;
            }
            queue.push(child.pid);
            if child.name.eq_ignore_ascii_case(WEBVIEW_EXE) {
                summed.count += 1;
                summed.rss_kib += child.rss_kib;
                summed.largest_kib = summed.largest_kib.max(child.rss_kib);
            }
        }
    }

    summed
}

/// Samples until the process ends.
///
/// Debug builds only: in a release build these figures would be written for a user who has
/// no reason to read them, and P0 is a diagnosis rather than a feature.
pub fn spawn(root: u32) {
    if !cfg!(debug_assertions) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let mut system = sysinfo::System::new();
        let mut ticks = tokio::time::interval(SAMPLE_EVERY);

        loop {
            ticks.tick().await;
            system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::All,
                true,
                sysinfo::ProcessRefreshKind::nothing().with_memory(),
            );
            crate::profile::processes(&totals(&snapshot(&system), root));
        }
    });
}

fn snapshot(system: &sysinfo::System) -> Vec<Process> {
    system
        .processes()
        .values()
        .map(|process| Process {
            pid: process.pid().as_u32(),
            parent: process.parent().map(sysinfo::Pid::as_u32),
            name: process.name().to_string_lossy().into_owned(),
            rss_kib: process.memory() / 1024,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, parent: u32, name: &str, rss_kib: u64) -> Process {
        Process {
            pid,
            parent: Some(parent),
            name: name.to_owned(),
            rss_kib,
        }
    }

    #[test]
    fn an_app_with_no_webview_sums_to_nothing() {
        assert_eq!(totals(&[], 100), Totals::default());
    }

    #[test]
    fn the_browser_process_is_counted() {
        let tree = [process(200, 100, "msedgewebview2.exe", 40_000)];
        let summed = totals(&tree, 100);
        assert_eq!(summed.count, 1);
        assert_eq!(summed.rss_kib, 40_000);
    }

    #[test]
    fn the_renderer_one_level_down_is_counted_too() {
        let tree = [
            process(200, 100, "msedgewebview2.exe", 40_000),
            process(300, 200, "msedgewebview2.exe", 900_000),
        ];
        let summed = totals(&tree, 100);
        assert_eq!(summed.count, 2);
        assert_eq!(summed.rss_kib, 940_000);
        assert_eq!(summed.largest_kib, 900_000);
    }

    #[test]
    fn another_apps_webview_is_not_our_memory() {
        let tree = [
            process(200, 100, "msedgewebview2.exe", 40_000),
            process(900, 800, "msedgewebview2.exe", 2_000_000),
        ];
        assert_eq!(totals(&tree, 100).rss_kib, 40_000);
    }

    #[test]
    fn our_own_other_children_are_not_counted() {
        let tree = [
            process(200, 100, "msedgewebview2.exe", 40_000),
            process(201, 100, "git.exe", 8_000),
        ];
        assert_eq!(totals(&tree, 100).count, 1);
    }

    #[test]
    fn a_reused_pid_pointing_back_into_the_walk_still_terminates() {
        let tree = [
            process(200, 100, "msedgewebview2.exe", 10),
            process(100, 200, "cogit.exe", 10),
        ];
        assert_eq!(totals(&tree, 100).count, 1);
    }

    #[test]
    fn the_name_is_matched_the_way_windows_compares_it() {
        let tree = [process(200, 100, "MsEdgeWebView2.exe", 1_234)];
        assert_eq!(totals(&tree, 100).count, 1);
    }
}
