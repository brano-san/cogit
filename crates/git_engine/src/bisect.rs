//! `git bisect`: read from `BISECT_*` and `refs/bisect/*`, stepped through git (M11).

use crate::{GitError, Head, RepoHandle, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BisectState {
    /// What `git bisect reset` goes back to: a branch, or a commit if it began detached.
    pub start: String,
    pub bad: Option<String>,
    pub good: Vec<String>,
    pub skipped: Vec<String>,
    /// The commit under test: HEAD, or `BISECT_HEAD` after `--no-checkout`.
    pub current: Option<String>,
    pub first_bad: Option<String>,
    /// Only skipped commits were left: the first bad one is one of these.
    pub candidates: Vec<String>,
    pub terms: BisectTerms,
}

/// The words `git bisect start --term-new/--term-old` chose; git refuses the others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BisectTerms {
    pub bad: String,
    pub good: String,
}

impl Default for BisectTerms {
    fn default() -> Self {
        Self {
            bad: "bad".to_owned(),
            good: "good".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum BisectMark {
    Good,
    Bad,
    Skip,
}

const LOG: &str = "BISECT_LOG";

impl RepoHandle {
    pub(crate) fn is_bisecting(&self) -> bool {
        self.git_dir().join(LOG).exists()
    }

    /// Best effort: a file or ref that cannot be read leaves its field empty, so the
    /// banner still shows and Reset still works.
    pub(crate) fn bisect_state(&self) -> BisectState {
        let git_dir = self.git_dir();
        let read = |name: &str| std::fs::read_to_string(git_dir.join(name)).unwrap_or_default();
        let terms = terms_of(&read("BISECT_TERMS"));
        let (first_bad, candidates) = outcome(&read(LOG), &terms.bad);
        let mut state = BisectState {
            start: read("BISECT_START").trim().to_owned(),
            current: self.bisect_current(&read("BISECT_HEAD")),
            first_bad,
            candidates,
            ..BisectState::default()
        };
        self.read_bisect_refs(&terms, &mut state);
        state.terms = terms;
        state
    }

    fn bisect_current(&self, bisect_head: &str) -> Option<String> {
        let detached = bisect_head.trim();
        if !detached.is_empty() {
            return Some(detached.to_owned());
        }
        match self.head() {
            Ok(Head::Branch { oid, .. } | Head::Detached { oid }) => Some(oid),
            Ok(Head::Unborn { .. }) => None,
            Err(error) => {
                tracing::error!(?error, context = "reading the commit under bisect");
                None
            }
        }
    }

    fn read_bisect_refs(&self, terms: &BisectTerms, state: &mut BisectState) {
        let refs = match self.repo.references() {
            Ok(refs) => refs,
            Err(error) => {
                tracing::error!(?error, context = "reading refs/bisect");
                return;
            }
        };
        let listed = match refs.prefixed("refs/bisect/") {
            Ok(listed) => listed,
            Err(error) => {
                tracing::error!(?error, context = "listing refs/bisect");
                return;
            }
        };
        let good = format!("{}-", terms.good);
        for reference in listed.filter_map(std::result::Result::ok) {
            let name = reference.name().as_bstr().to_string();
            let Some(name) = name.strip_prefix("refs/bisect/") else {
                continue;
            };
            let Some(oid) = reference.try_id().map(|id| id.detach().to_string()) else {
                continue;
            };
            if name == terms.bad {
                state.bad = Some(oid);
            } else if name.starts_with(&good) {
                state.good.push(oid);
            } else if name.starts_with("skip-") {
                state.skipped.push(oid);
            }
        }
    }

    /// Commits are resolved here, so git gets ids and `--` keeps them from being paths.
    pub fn bisect_start(&self, bad: &str, good: Option<&str>) -> Result<()> {
        if self.is_bare() {
            return Err(GitError::InvalidState(
                "a bare repository has no working tree to test commits in".to_owned(),
            ));
        }
        if self.is_bisecting() {
            return Err(GitError::InvalidState(
                "a bisect is already in progress".to_owned(),
            ));
        }
        let bad = self.resolve_commit(bad)?.to_string();
        let good = good.map(|rev| self.resolve_commit(rev)).transpose()?;
        let good = good.map(|oid| oid.to_string());
        if good.as_ref() == Some(&bad) {
            return Err(GitError::InvalidState(
                "the good and the bad commit are the same commit".to_owned(),
            ));
        }
        let mut args = vec!["bisect", "start", bad.as_str()];
        args.extend(good.as_deref());
        args.push("--");
        self.run_git(&args).map(drop)
    }

    /// `rev` is HEAD when `None`; git then checks out the next commit to test.
    pub fn bisect_mark(&self, mark: BisectMark, rev: Option<&str>) -> Result<()> {
        if !self.is_bisecting() {
            return Err(GitError::InvalidState(
                "no bisect is in progress".to_owned(),
            ));
        }
        let terms = terms_of(
            &std::fs::read_to_string(self.git_dir().join("BISECT_TERMS")).unwrap_or_default(),
        );
        let term = match mark {
            BisectMark::Good => terms.good,
            BisectMark::Bad => terms.bad,
            BisectMark::Skip => "skip".to_owned(),
        };
        let oid = rev.map(|rev| self.resolve_commit(rev)).transpose()?;
        let oid = oid.map(|oid| oid.to_string());
        let mut args = vec!["bisect", term.as_str()];
        args.extend(oid.as_deref());
        self.run_git(&args).map(drop)
    }

    pub fn bisect_reset(&self) -> Result<()> {
        if !self.is_bisecting() {
            return Err(GitError::InvalidState(
                "no bisect is in progress".to_owned(),
            ));
        }
        self.run_git(&["bisect", "reset"]).map(drop)
    }
}

fn terms_of(file: &str) -> BisectTerms {
    let mut lines = file.lines().map(str::trim).filter(|line| !line.is_empty());
    match (lines.next(), lines.next()) {
        (Some(bad), Some(good)) => BisectTerms {
            bad: bad.to_owned(),
            good: good.to_owned(),
        },
        _ => BisectTerms::default(),
    }
}

/// What git wrote to the log after its last `git bisect …` line: `# first bad commit`
/// when the search is over, `# possible first bad commit` lines when only skipped
/// commits were left (git 2.51 writes both).
fn outcome(log: &str, bad: &str) -> (Option<String>, Vec<String>) {
    let tail: Vec<&str> = log
        .lines()
        .rev()
        .take_while(|line| !line.starts_with("git bisect "))
        .collect();
    let first = format!("# first {bad} commit: [");
    let possible = format!("# possible first {bad} commit: [");
    let mut found = None;
    let mut candidates = Vec::new();
    for line in tail.into_iter().rev() {
        if let Some(rest) = line.strip_prefix(&first) {
            found = bracketed_oid(rest);
        } else if let Some(oid) = line.strip_prefix(&possible).and_then(bracketed_oid) {
            candidates.push(oid);
        }
    }
    (found, candidates)
}

fn bracketed_oid(rest: &str) -> Option<String> {
    let hex = rest.split(']').next()?;
    (hex.len() >= 40 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hex.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "b1d592c4e86d8a2ebf9ab43e110a42c0d0ee2a03";
    const B: &str = "21aeb4b6b34bd9cef79208536ccedd7c3e555e68";

    #[test]
    fn the_first_bad_commit_is_read_after_the_last_step() {
        let log = format!(
            "git bisect start 'HEAD' 'HEAD~7' '--'\n# bad: [{A}] c1\ngit bisect bad {A}\n# first bad commit: [{A}] c1\n"
        );
        assert_eq!(outcome(&log, "bad"), (Some(A.to_owned()), Vec::new()));
    }

    #[test]
    fn a_step_after_the_end_starts_the_search_again() {
        let log = format!("# first bad commit: [{A}] c1\ngit bisect good {B}\n");
        assert_eq!(outcome(&log, "bad"), (None, Vec::new()));
    }

    #[test]
    fn only_skipped_commits_left_lists_the_candidates() {
        let log = format!(
            "git bisect skip {A}\n# only skipped commits left to test\n# possible first bad commit: [{A}] c1\n# possible first bad commit: [{B}] c3\n"
        );
        assert_eq!(
            outcome(&log, "bad"),
            (None, vec![A.to_owned(), B.to_owned()])
        );
    }

    #[test]
    fn a_bisect_with_its_own_terms_names_the_end_in_them() {
        let log = format!("git bisect broken {A}\n# first broken commit: [{A}] c1\n");
        assert_eq!(outcome(&log, "broken").0.as_deref(), Some(A));
        assert_eq!(outcome(&log, "bad").0, None);
    }

    #[test]
    fn terms_default_to_bad_and_good() {
        assert_eq!(terms_of(""), BisectTerms::default());
        assert_eq!(
            terms_of("broken\nfixed\n"),
            BisectTerms {
                bad: "broken".to_owned(),
                good: "fixed".to_owned()
            }
        );
    }
}
