//! Health checks on the object store and the checkout's shape, and the maintenance a user
//! can ask for.

use crate::{GitError, HealthIssue, RepoHandle, Result};
use std::fmt::Write as _;
use std::io::Read;
use unicode_normalization::UnicodeNormalization;

const LFS_POINTER: &[u8] = b"version https://git-lfs.github.com/spec/v1";
const STALE_GC_LOCK_HOURS: u64 = 12;
const STALE_GRAPH_LOCK_MINUTES: u64 = 10;
const GRAPH_WORTH_OBJECTS: u64 = 10_000;
const SAMPLE: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MaintenanceTask {
    Gc,
    CommitGraph,
    ClearGcLock,
    ClearCommitGraphLock,
}

/// How old an unreachable object must be for `git gc` to delete it; asked each time.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum GcPrune {
    /// Git's own default.
    #[default]
    TwoWeeks,
    OneWeek,
    OneDay,
    OneHour,
    Now,
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GcOptions {
    pub prune: GcPrune,
    pub clear_stashes: bool,
    pub clear_lost_commits: bool,
    pub clear_recovery_stashes: bool,
}

impl GcPrune {
    fn arg(self) -> &'static str {
        match self {
            Self::TwoWeeks => "--prune=2.weeks.ago",
            Self::OneWeek => "--prune=1.week.ago",
            Self::OneDay => "--prune=1.day.ago",
            Self::OneHour => "--prune=1.hour.ago",
            Self::Now => "--prune=now",
        }
    }

    /// Given to gc as config, not to `reflog expire` on the command line: that bypasses
    /// git's own `refs/stash` exemption and every stash but the top one was pruned.
    fn reflog_expiry(self) -> Option<&'static str> {
        match self {
            Self::TwoWeeks => None,
            Self::OneWeek => Some("gc.reflogExpireUnreachable=1.week.ago"),
            Self::OneDay => Some("gc.reflogExpireUnreachable=1.day.ago"),
            Self::OneHour => Some("gc.reflogExpireUnreachable=1.hour.ago"),
            Self::Now => Some("gc.reflogExpireUnreachable=now"),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ObjectCounts {
    loose: u64,
    in_pack: u64,
    packs: u64,
    pack_kib: u64,
}

impl RepoHandle {
    /// `git count-objects -v` read from the object directory itself, on every open, without
    /// a process: loose objects by their file names, packs by their `.idx` fan-out (whose
    /// last entry is the object count), `size-pack` as pack plus index bytes. `None` when
    /// the directory or a pack index cannot be read; git is asked then.
    fn object_counts(&self) -> Option<ObjectCounts> {
        let objects = self.repo.objects.store_ref().path().to_path_buf();
        let hex_len = self.repo.object_hash().len_in_hex() - 2;
        let hex_name = |name: &std::ffi::OsStr, len: usize| {
            let name = name.to_string_lossy();
            name.len() == len && name.bytes().all(|b| b.is_ascii_hexdigit())
        };
        let mut counts = ObjectCounts::default();
        for entry in std::fs::read_dir(&objects).ok()?.flatten() {
            if !hex_name(&entry.file_name(), 2) {
                continue;
            }
            let Ok(files) = std::fs::read_dir(entry.path()) else {
                continue;
            };
            counts.loose += files
                .flatten()
                .filter(|file| hex_name(&file.file_name(), hex_len))
                .count() as u64;
        }
        let mut pack_bytes = 0_u64;
        if let Ok(packs) = std::fs::read_dir(objects.join("pack")) {
            for entry in packs.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|ext| ext != "pack") {
                    continue;
                }
                let index = path.with_extension("idx");
                let Ok(index_meta) = std::fs::metadata(&index) else {
                    continue;
                };
                counts.in_pack += index_object_count(&index)?;
                counts.packs += 1;
                pack_bytes += entry.metadata().map_or(0, |meta| meta.len()) + index_meta.len();
            }
        }
        counts.pack_kib = pack_bytes / 1024;
        Some(counts)
    }
}

/// The object count of a pack index: the last of its 256 fan-out entries, after the v2
/// header (magic `ff 74 4f 63`, then the version) or at the very start of a v1 index.
fn index_object_count(index: &std::path::Path) -> Option<u64> {
    use std::io::{Read as _, Seek as _, SeekFrom};
    let mut file = std::fs::File::open(index).ok()?;
    let mut head = [0_u8; 8];
    file.read_exact(&mut head).ok()?;
    let fanout = if head[..4] == [0xff, b't', b'O', b'c'] {
        8
    } else {
        0
    };
    file.seek(SeekFrom::Start(fanout + 255 * 4)).ok()?;
    let mut last = [0_u8; 4];
    file.read_exact(&mut last).ok()?;
    Some(u64::from(u32::from_be_bytes(last)))
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
        issues.extend(self.commit_graph_lock_issue());
        if !self.repo.is_bare() {
            issues.extend(self.sparse_issue());
            issues.extend(self.checkout_path_issues());
        }
        issues
    }

    fn object_issues(&self) -> Vec<HealthIssue> {
        let counts = match self.object_counts() {
            Some(counts) => counts,
            // `UnsafeDirectory` says it already; count-objects would only add a refusal.
            None if self.git_refuses() => return Vec::new(),
            None => match self.read_git(&["count-objects", "-v"]) {
                Ok(text) => parse_count_objects(&text),
                Err(err) => {
                    tracing::warn!(error = ?err, context = "count-objects for health");
                    return Vec::new();
                }
            },
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

    fn commit_graph_locks(&self) -> Vec<std::path::PathBuf> {
        let info = self.repo.objects.store_ref().path().join("info");
        [
            info.join("commit-graph.lock"),
            info.join("commit-graphs").join("commit-graph-chain.lock"),
        ]
        .into_iter()
        .filter(|lock| lock.exists())
        .collect()
    }

    fn stale_commit_graph_locks(&self) -> Vec<(std::path::PathBuf, u64)> {
        self.commit_graph_locks()
            .into_iter()
            .filter_map(|lock| {
                let minutes = std::fs::metadata(&lock)
                    .ok()?
                    .modified()
                    .ok()?
                    .elapsed()
                    .ok()?
                    .as_secs()
                    / 60;
                (minutes >= STALE_GRAPH_LOCK_MINUTES).then_some((lock, minutes))
            })
            .collect()
    }

    fn commit_graph_lock_issue(&self) -> Option<HealthIssue> {
        let minutes = self
            .stale_commit_graph_locks()
            .into_iter()
            .map(|(_, minutes)| minutes)
            .max()?;
        Some(HealthIssue::StaleCommitGraphLock { minutes })
    }

    fn refuse_under_commit_graph_lock(&self) -> Result<()> {
        match self.commit_graph_locks().first() {
            None => Ok(()),
            Some(lock) => Err(GitError::InvalidState(format!(
                "{} is there: a git commit-graph or git gc is writing the commit-graph, or one stopped and left it. Remove it in the repository health check once nothing is running.",
                lock.file_name()
                    .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
            ))),
        }
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

    pub fn gc(&self, prune: GcPrune) -> Result<()> {
        self.gc_with(
            GcOptions {
                prune,
                ..GcOptions::default()
            },
            &[],
            |_| {},
        )
    }

    /// Objects in the live Undo journal include blobs that Git cannot otherwise reach.
    /// Temporary refs keep them through a repack, including when a subsequent step fails.
    pub fn gc_with(
        &self,
        options: GcOptions,
        undo_blobs: &[String],
        mut on_line: impl FnMut(&str),
    ) -> Result<()> {
        self.refuse_under_commit_graph_lock()?;
        let prune = if options.clear_lost_commits {
            GcPrune::Now
        } else {
            options.prune
        };
        let destructive =
            prune == GcPrune::Now || options.clear_stashes || options.clear_recovery_stashes;
        let git_dirs = if destructive {
            self.maintenance_git_dirs()?
        } else {
            Vec::new()
        };
        if destructive {
            self.require_idle_for_cleanup(&git_dirs)?;
        }

        let prefix = format!("refs/cogit/maintenance/{}/", std::process::id());
        let mut create = String::new();
        let mut delete = String::new();
        let mut seen = std::collections::HashSet::new();
        for oid in undo_blobs
            .iter()
            .filter_map(|oid| gix::ObjectId::from_hex(oid.as_bytes()).ok())
        {
            if seen.insert(oid)
                && self
                    .repo
                    .find_header(oid)
                    .is_ok_and(|header| header.kind() == gix::object::Kind::Blob)
            {
                let _ = writeln!(create, "create {prefix}{oid} {oid}");
                let _ = writeln!(delete, "delete {prefix}{oid} {oid}");
            }
        }
        if create.is_empty() {
            return self.gc_steps(options, prune, &git_dirs, &mut on_line);
        }
        let pin_args = [
            "-c",
            "core.logAllRefUpdates=false",
            "update-ref",
            "--no-deref",
            "--stdin",
        ];
        self.run_git_fed(&pin_args, create.as_bytes())?;
        let result = self.gc_steps(options, prune, &git_dirs, &mut on_line);
        let release = self.run_git_fed(&pin_args, delete.as_bytes());
        result.and(release.map(drop))
    }

    fn gc_steps(
        &self,
        options: GcOptions,
        prune: GcPrune,
        git_dirs: &[std::path::PathBuf],
        on_line: &mut impl FnMut(&str),
    ) -> Result<()> {
        if options.clear_stashes {
            on_line("Clearing saved stashes…");
            self.run_git(&["stash", "clear"])?;
        }
        if options.clear_lost_commits || options.clear_recovery_stashes {
            on_line("Checking Cogit recovery references…");
            self.clear_gc_backups(options)?;
        }
        if options.clear_lost_commits {
            on_line("Expiring unreachable reflog entries…");
            self.expire_lost_reflogs(git_dirs)?;
        }
        on_line("Packing objects and pruning unreachable objects…");
        let mut args = vec![
            "-c",
            "gc.writeCommitGraph=false",
            "-c",
            "gc.refs/stash.reflogExpire=never",
            "-c",
            "gc.refs/stash.reflogExpireUnreachable=never",
        ];
        if options.clear_lost_commits {
            // The explicit pass above replaces Git's default reflog expiration for this run.
            args.extend([
                "-c",
                "gc.reflogExpire=never",
                "-c",
                "gc.reflogExpireUnreachable=never",
            ]);
        } else if let Some(expiry) = prune.reflog_expiry() {
            args.extend(["-c", expiry]);
        }
        args.extend(["gc", "--no-quiet", prune.arg()]);
        self.stream_maintenance(&args, &mut *on_line)?;
        self.write_commit_graph(on_line)
    }

    fn maintenance_git_dirs(&self) -> Result<Vec<std::path::PathBuf>> {
        let mut dirs = vec![self.common_dir().to_owned()];
        let worktrees = self
            .repo
            .worktrees()
            .map_err(|err| GitError::Io(format!("cannot list worktrees: {err}")))?;
        dirs.extend(worktrees.iter().map(|proxy| proxy.git_dir().to_owned()));
        Ok(dirs)
    }

    fn require_idle_for_cleanup(&self, git_dirs: &[std::path::PathBuf]) -> Result<()> {
        if self.common_dir().join("gc.pid").exists() {
            return Err(GitError::InvalidState(
                "git gc is already running or its lock is still present".to_owned(),
            ));
        }
        const OPERATIONS: &[&str] = &[
            "MERGE_HEAD",
            "MERGE_AUTOSTASH",
            "rebase-merge",
            "rebase-apply",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "sequencer",
            "BISECT_START",
        ];
        if git_dirs
            .iter()
            .any(|dir| OPERATIONS.iter().any(|name| dir.join(name).exists()))
        {
            return Err(GitError::InvalidState(
                "finish or abort the Git operation in progress before cleaning up".to_owned(),
            ));
        }
        Ok(())
    }

    fn clear_gc_backups(&self, options: GcOptions) -> Result<()> {
        use crate::stash::BACKUP_REFS;
        let refs = self.read_git(&[
            "for-each-ref",
            "--format=%(objectname) %(refname)",
            BACKUP_REFS,
        ])?;
        if refs.is_empty() {
            return Ok(());
        }
        let reachable = self.reachable_from(&self.keeping_tips()?)?;
        let mut delete = String::new();
        for (oid, name) in refs.lines().filter_map(|line| line.split_once(' ')) {
            if name.strip_prefix(BACKUP_REFS) != Some(oid) {
                continue;
            }
            let Ok(id) = gix::ObjectId::from_hex(oid.as_bytes()) else {
                continue;
            };
            let Some(commit) = self
                .repo
                .find_object(id)
                .ok()
                .and_then(|object| object.peel_to_commit().ok())
            else {
                continue;
            };
            if reachable.contains(&commit.id) {
                continue;
            }
            let stash = self.is_recovery_stash(&commit);
            if (stash && options.clear_recovery_stashes) || (!stash && options.clear_lost_commits) {
                let _ = writeln!(delete, "delete {name} {oid}");
            }
        }
        if !delete.is_empty() {
            self.run_git_fed(&["update-ref", "--no-deref", "--stdin"], delete.as_bytes())?;
        }
        Ok(())
    }

    /// Git's stash index commit has the base as its sole parent. Git writes its subject
    /// as "index on …" without translation (builtin/stash.c). The topology alone also
    /// matches some ordinary merge commits.
    fn is_recovery_stash(&self, commit: &gix::Commit<'_>) -> bool {
        let mut parents = commit.parent_ids();
        let (Some(base), Some(index)) = (parents.next(), parents.next()) else {
            return false;
        };
        let Ok(index) = self.repo.find_commit(index.detach()) else {
            return true;
        };
        index.parent_ids().map(gix::Id::detach).eq([base.detach()])
            && index
                .message_raw()
                .map_or(true, |message| message.starts_with(b"index on "))
    }

    /// `--all` would also expire the stash history, even with the normal config exemption.
    /// List the logs for each worktree and use explicit expiration so repository patterns
    /// such as gc.refs/heads/*.reflogExpireUnreachable cannot retain lost commits.
    fn expire_lost_reflogs(&self, git_dirs: &[std::path::PathBuf]) -> Result<()> {
        for (i, dir) in git_dirs.iter().enumerate() {
            let dir = crate::slash_path(dir);
            let logs = self.read_git(&["--git-dir", &dir, "reflog", "list"])?;
            let names: Vec<String> = logs
                .lines()
                .filter(|&name| {
                    name != "refs/stash"
                        && (i == 0
                            || name == "HEAD"
                            || ["refs/bisect/", "refs/worktree/", "refs/rewritten/"]
                                .iter()
                                .any(|prefix| name.starts_with(prefix)))
                })
                .map(str::to_owned)
                .collect();
            for batch in crate::runner::command_line_batches(&names) {
                let mut args = vec![
                    "--git-dir",
                    &dir,
                    "reflog",
                    "expire",
                    "--expire=never",
                    "--expire-unreachable=now",
                    "--",
                ];
                args.extend(batch.iter().map(String::as_str));
                self.run_git(&args)?;
            }
        }
        Ok(())
    }

    fn stream_maintenance(&self, args: &[&str], on_line: impl FnMut(&str)) -> Result<()> {
        let stream = crate::network::Streamed {
            root: self.root(),
            stop: None,
            journal: &|entry| self.journal_entry(entry),
        };
        let mut git = self.base_git(args);
        git.env("GIT_PROGRESS_DELAY", "0");
        stream.run_local(git, args, on_line)
    }

    /// An Undo record may also store a branch name. Only literal IDs can be pruned.
    #[must_use]
    pub fn maintenance_object_missing(&self, revision: &str) -> bool {
        gix::ObjectId::from_hex(revision.as_bytes()).is_ok_and(|oid| !self.repo.has_object(oid))
    }

    fn write_commit_graph(&self, mut on_line: impl FnMut(&str)) -> Result<()> {
        on_line("Writing commit graph…");
        self.stream_maintenance(
            &[
                "commit-graph",
                "write",
                "--reachable",
                "--changed-paths",
                "--progress",
            ],
            on_line,
        )
    }

    /// `prune` matters to `Gc` only.
    pub fn run_maintenance(&self, task: MaintenanceTask, prune: GcPrune) -> Result<()> {
        self.run_maintenance_with(
            task,
            GcOptions {
                prune,
                ..GcOptions::default()
            },
            &[],
            |_| {},
        )
    }

    pub fn run_maintenance_with(
        &self,
        task: MaintenanceTask,
        options: GcOptions,
        undo_blobs: &[String],
        mut on_line: impl FnMut(&str),
    ) -> Result<()> {
        match task {
            MaintenanceTask::Gc => self.gc_with(options, undo_blobs, on_line),
            MaintenanceTask::CommitGraph => {
                self.refuse_under_commit_graph_lock()?;
                self.write_commit_graph(on_line)
            }
            MaintenanceTask::ClearCommitGraphLock => {
                on_line("Removing stale commit-graph locks…");
                let stale = self.stale_commit_graph_locks();
                if stale.is_empty() {
                    return Err(GitError::InvalidState(
                        "the commit-graph lock is recent or gone: a commit-graph write may still be running".to_owned(),
                    ));
                }
                for (lock, _) in stale {
                    std::fs::remove_file(&lock).map_err(|err| {
                        GitError::Internal(format!("cannot remove {}: {err}", lock.display()))
                    })?;
                }
                Ok(())
            }
            MaintenanceTask::ClearGcLock => {
                on_line("Removing the stale gc lock…");
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

    /// The object directory read by hand says what `git count-objects -v` says, with loose
    /// objects only, with packs only, and with both.
    #[test]
    fn object_counts_match_git_count_objects() {
        let f = test_fixtures::linear(5).unwrap();
        let check = |when: &str| {
            let repo = RepoHandle::open(f.path()).unwrap();
            let git = parse_count_objects(&f.git(&["count-objects", "-v"]).unwrap());
            assert_eq!(repo.object_counts(), Some(git), "{when}");
        };
        check("loose only");
        f.git(&["gc", "--quiet"]).unwrap();
        check("packed");
        f.commit_file(9, "later.txt", "later\n").unwrap();
        check("packs and loose");
    }

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
