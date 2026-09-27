//! Health checks on the object store and the checkout's shape, and the maintenance a user
//! can ask for.

use crate::{GitError, HealthIssue, RepoHandle, Result};
use std::io::Read;
use unicode_normalization::UnicodeNormalization;

const LFS_POINTER: &[u8] = b"version https://git-lfs.github.com/spec/v1";
const STALE_GC_LOCK_HOURS: u64 = 12;
const GRAPH_WORTH_OBJECTS: u64 = 10_000;
const SAMPLE: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MaintenanceTask {
    Gc,
    CommitGraph,
    ClearGcLock,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ObjectCounts {
    loose: u64,
    in_pack: u64,
    packs: u64,
    pack_kib: u64,
}

fn parse_count_objects(text: &str) -> ObjectCounts {
    let mut counts = ObjectCounts::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().parse().unwrap_or(0);
        match key {
            "count" => counts.loose = value,
            "in-pack" => counts.in_pack = value,
            "packs" => counts.packs = value,
            "size-pack" => counts.pack_kib = value,
            _ => {}
        }
    }
    counts
}

/// Paths the index holds in two Unicode forms that one file system sees as one name.
fn normalization_twins<'a>(paths: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen = std::collections::HashMap::<String, &str>::new();
    let mut twins = Vec::new();
    for path in paths {
        if path.is_ascii() {
            continue;
        }
        let nfc: String = path.nfc().collect();
        match seen.get(nfc.as_str()) {
            Some(other) if *other != path => twins.push(path.to_owned()),
            Some(_) => {}
            None => {
                seen.insert(nfc, path);
            }
        }
    }
    twins
}

impl RepoHandle {
    pub(crate) fn housekeeping_issues(&self) -> Vec<HealthIssue> {
        let mut issues = Vec::new();
        issues.extend(self.object_issues());
        issues.extend(self.gc_lock_issue());
        if !self.repo.is_bare() {
            issues.extend(self.sparse_issue());
            issues.extend(self.checkout_path_issues());
        }
        issues
    }

    fn object_issues(&self) -> Vec<HealthIssue> {
        let counts = match self.read_git(&["count-objects", "-v"]) {
            Ok(text) => parse_count_objects(&text),
            Err(err) => {
                tracing::warn!(error = ?err, context = "count-objects for health");
                return Vec::new();
            }
        };
        let config = self.repo.config_snapshot();
        let int = |key: &str, default: u64| {
            config
                .integer(key)
                .and_then(|value| u64::try_from(value).ok())
                .unwrap_or(default)
        };
        let loose_limit = int("gc.auto", 6700);
        let pack_limit = int("gc.autoPackLimit", 50);
        let mut issues = Vec::new();
        if (loose_limit > 0 && counts.loose > loose_limit)
            || (pack_limit > 0 && counts.packs > pack_limit)
        {
            issues.push(HealthIssue::HousekeepingDue {
                loose: counts.loose,
                packs: counts.packs,
                pack_bytes: counts.pack_kib * 1024,
            });
        }
        let info = self.repo.objects.store_ref().path().join("info");
        let has_graph = info.join("commit-graph").exists() || info.join("commit-graphs").is_dir();
        if !has_graph
            && config.boolean("core.commitGraph") != Some(false)
            && counts.in_pack + counts.loose >= GRAPH_WORTH_OBJECTS
        {
            issues.push(HealthIssue::NoCommitGraph);
        }
        issues
    }

    fn gc_lock_issue(&self) -> Option<HealthIssue> {
        let modified = std::fs::metadata(self.repo.common_dir().join("gc.pid"))
            .ok()?
            .modified()
            .ok()?;
        let hours = modified.elapsed().ok()?.as_secs() / 3600;
        (hours >= STALE_GC_LOCK_HOURS).then_some(HealthIssue::StaleGcLock { hours })
    }

    fn sparse_issue(&self) -> Option<HealthIssue> {
        if !self.sparse_checkout() {
            return None;
        }
        let config = self.repo.config_snapshot();
        let file = self.repo.git_dir().join("info").join("sparse-checkout");
        let patterns = std::fs::read_to_string(file)
            .map(|text| {
                text.lines()
                    .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
                    .count()
            })
            .unwrap_or(0);
        Some(HealthIssue::SparseCheckout {
            cone: config.boolean("core.sparseCheckoutCone").unwrap_or(true),
            patterns: u32::try_from(patterns).unwrap_or(u32::MAX),
        })
    }

    /// One pass over the index for what depends on the files' names and first bytes.
    fn checkout_path_issues(&self) -> Vec<HealthIssue> {
        let Ok(index) = self.current_index() else {
            return Vec::new();
        };
        let paths: Vec<String> = index
            .entries()
            .iter()
            .map(|entry| entry.path(&index).to_string())
            .collect();
        let mut issues = Vec::new();

        let twins = normalization_twins(paths.iter().map(String::as_str));
        if !twins.is_empty() {
            issues.push(HealthIssue::NormalizationTwins {
                paths: twins.into_iter().take(SAMPLE).collect(),
            });
        }
        if cfg!(target_os = "macos")
            && self
                .repo
                .config_snapshot()
                .boolean("core.precomposeUnicode")
                == Some(false)
            && paths.iter().any(|path| !path.is_ascii())
        {
            issues.push(HealthIssue::PrecomposeUnicodeOff);
        }

        let uses_lfs = paths
            .iter()
            .filter(|path| path.rsplit('/').next() == Some(".gitattributes"))
            .any(|path| {
                std::fs::read_to_string(self.root().join(path))
                    .is_ok_and(|text| text.contains("filter=lfs"))
            });
        if uses_lfs {
            let mut attributes = self.diff_attributes();
            let pointers: Vec<&String> = paths
                .iter()
                .filter(|path| attributes.value(path, "filter").as_deref() == Some("lfs"))
                .filter(|path| self.is_lfs_pointer_on_disk(path))
                .collect();
            if !pointers.is_empty() {
                issues.push(HealthIssue::LfsPointers {
                    count: u32::try_from(pointers.len()).unwrap_or(u32::MAX),
                    installed: crate::lfs_version().is_some(),
                    sample: pointers.into_iter().take(SAMPLE).cloned().collect(),
                });
            }
        }
        issues
    }

    #[must_use]
    pub fn is_lfs_pointer_on_disk(&self, path: &str) -> bool {
        let mut head = [0u8; LFS_POINTER.len()];
        std::fs::File::open(self.root().join(path))
            .and_then(|mut file| file.read_exact(&mut head))
            .is_ok_and(|()| head == LFS_POINTER)
    }

    pub fn run_maintenance(&self, task: MaintenanceTask) -> Result<()> {
        match task {
            MaintenanceTask::Gc => self.run_git(&["gc"]).map(drop),
            MaintenanceTask::CommitGraph => self
                .run_git(&["commit-graph", "write", "--reachable", "--changed-paths"])
                .map(drop),
            MaintenanceTask::ClearGcLock => {
                if self.gc_lock_issue().is_none() {
                    return Err(GitError::InvalidState(
                        "gc.pid is recent or gone: a gc may still be running".to_owned(),
                    ));
                }
                std::fs::remove_file(self.repo.common_dir().join("gc.pid"))
                    .map_err(|err| GitError::Internal(format!("cannot remove gc.pid: {err}")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_objects_verbose_is_read() {
        let text =
            "count: 12\nsize: 48\nin-pack: 3400\npacks: 2\nsize-pack: 900\nprune-packable: 0\n";
        assert_eq!(
            parse_count_objects(text),
            ObjectCounts {
                loose: 12,
                in_pack: 3400,
                packs: 2,
                pack_kib: 900
            }
        );
    }

    #[test]
    fn a_name_in_both_unicode_forms_is_a_twin() {
        let nfc = "caf\u{e9}.txt";
        let nfd = "cafe\u{301}.txt";
        assert_eq!(normalization_twins([nfc, nfd, "a.txt"].into_iter()), [nfd]);
        assert!(normalization_twins([nfc, "b.txt"].into_iter()).is_empty());
    }
}
