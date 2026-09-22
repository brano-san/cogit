//! Repository warnings; nothing here repairs anything (doc/12-risks.md, R-150).

use crate::gitlink::is_foreign_path;
use crate::{GitError, ModuleProblem, RepoHandle};
use serde::Serialize;
use std::path::Path;

const MAX_DEPTH: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HealthIssue {
    /// `configured` is what `core.ignoreCase` says, `actual` what the folder does.
    IgnoreCaseMismatch {
        configured: bool,
        actual: bool,
    },
    DanglingModule {
        target: String,
        foreign: bool,
    },
    DanglingWorktree {
        name: String,
        target: String,
        foreign: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HealthFinding {
    /// Path from the repository that was checked; empty for that repository itself.
    pub module: String,
    pub issue: HealthIssue,
}

/// Tried, not assumed: WSL folders and case-sensitive NTFS directories exist.
pub fn case_sensitive(dir: &Path) -> std::io::Result<bool> {
    let name = format!("cogit-case-probe-{}", std::process::id());
    let lower = dir.join(&name);
    std::fs::write(&lower, b"")?;
    let other = dir.join(name.to_uppercase());
    let sensitive = !other.exists();
    std::fs::remove_file(&lower)?;
    Ok(sensitive)
}

impl RepoHandle {
    #[must_use]
    pub fn health_report(&self) -> Vec<HealthFinding> {
        let mut found = Vec::new();
        self.collect_health("", 0, &mut found);
        found
    }

    fn collect_health(&self, prefix: &str, depth: usize, found: &mut Vec<HealthFinding>) {
        for issue in self.own_health() {
            found.push(HealthFinding {
                module: prefix.to_owned(),
                issue,
            });
        }
        if depth >= MAX_DEPTH {
            return;
        }

        let Ok(modules) = self.submodules() else {
            return;
        };
        for module in modules {
            let key = if prefix.is_empty() {
                module.path.clone()
            } else {
                format!("{prefix}/{}", module.path)
            };
            match RepoHandle::open_exact(&self.root().join(&module.path)) {
                Ok(inner) => inner.collect_health(&key, depth + 1, found),
                Err(GitError::ModuleUnavailable(ModuleProblem::DanglingGitFile {
                    target,
                    foreign,
                    ..
                })) => found.push(HealthFinding {
                    module: key,
                    issue: HealthIssue::DanglingModule { target, foreign },
                }),
                // Not checked out is a state, not a fault; the tree already says so.
                Err(_) => {}
            }
        }
    }

    fn own_health(&self) -> Vec<HealthIssue> {
        let mut issues = Vec::new();
        if let Some(issue) = self.ignore_case_issue() {
            issues.push(issue);
        }
        issues.extend(self.worktree_issues());
        issues
    }

    fn ignore_case_issue(&self) -> Option<HealthIssue> {
        let configured = self
            .repo
            .config_snapshot()
            .boolean("core.ignoreCase")
            .unwrap_or(false);
        let actual = match case_sensitive(self.repo.git_dir()) {
            Ok(sensitive) => !sensitive,
            Err(err) => {
                tracing::warn!(error = %err, context = "case probe", "cannot probe the file system");
                return None;
            }
        };
        (configured != actual).then_some(HealthIssue::IgnoreCaseMismatch { configured, actual })
    }

    fn worktree_issues(&self) -> Vec<HealthIssue> {
        let Ok(entries) = std::fs::read_dir(self.repo.common_dir().join("worktrees")) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|entry| {
                let admin = entry.path();
                let target = std::fs::read_to_string(admin.join("gitdir")).ok()?;
                let target = target.trim().to_owned();
                (!Path::new(&target).exists()).then(|| HealthIssue::DanglingWorktree {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    foreign: is_foreign_path(&target),
                    target,
                })
            })
            .collect()
    }
}
