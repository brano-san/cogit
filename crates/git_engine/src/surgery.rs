use crate::{GitError, RepoHandle, Result};

impl RepoHandle {
    /// Without moving HEAD: the user stays on their branch, not in detached HEAD.
    pub fn rollback_to(&self, rev: &str, paths: &[String]) -> Result<()> {
        if rev.trim().is_empty() {
            return Err(GitError::InvalidState("no revision given".to_owned()));
        }

        let source = format!("--source={rev}");
        let everything = [".".to_owned()];
        let paths = if paths.is_empty() {
            &everything[..]
        } else {
            paths
        };
        self.run_git_paths(&["restore", source.as_str()], paths)
            .map(drop)
    }

    /// Splits `rev` in two. History from `rev` on is rewritten, so the caller warns first.
    pub fn split_off(
        &self,
        rev: &str,
        paths: &[String],
        message: &str,
        split_first: bool,
    ) -> Result<()> {
        let target = self.rev_parse(rev)?;
        let parent = self.only_parent(&target)?;
        let branch = self.current_branch()?;
        self.check_clean()?;
        self.check_subset(&target, paths)?;

        let original = self.rev_parse("HEAD")?;
        self.run_git(&["update-ref", "ORIG_HEAD", &original])?;

        match self.rewrite(&target, &parent, paths, message, split_first, &branch) {
            Ok(()) => Ok(()),
            Err(err) => {
                self.recover(&branch, &original);
                Err(err)
            }
        }
    }

    fn rewrite(
        &self,
        target: &str,
        parent: &str,
        paths: &[String],
        message: &str,
        split_first: bool,
        branch: &str,
    ) -> Result<()> {
        self.run_git(&["checkout", "--detach", parent])?;

        if split_first {
            self.materialise(target, paths)?;
            self.commit_with(&["-m", message])?;
            self.reset_tree_to(target)?;
            self.commit_with(&["-C", target])?;
        } else {
            self.reset_tree_to(target)?;
            self.materialise(parent, paths)?;
            self.commit_with(&["-C", target])?;
            self.reset_tree_to(target)?;
            self.commit_with(&["-m", message])?;
        }

        let tip = self.rev_parse("HEAD")?;
        self.run_git(&["rebase", "--onto", &tip, target, branch])
            .map(drop)
    }

    fn reset_tree_to(&self, rev: &str) -> Result<()> {
        self.run_git(&["read-tree", "-u", "--reset", rev]).map(drop)
    }

    /// Brings just `paths` to their state in `rev`, deleting the ones absent from it.
    fn materialise(&self, rev: &str, paths: &[String]) -> Result<()> {
        // `-z`: without it a non-ASCII name comes back quoted and escaped.
        let mut listing = vec!["ls-tree", "-r", "-z", "--name-only", rev, "--"];
        listing.extend(paths.iter().map(String::as_str));
        let present = nul_separated(&self.run_git(&listing)?.stdout);

        if !present.is_empty() {
            let mut args = vec!["checkout", rev, "--"];
            args.extend(present.iter().map(String::as_str));
            self.run_git(&args)?;
        }

        let removed: Vec<&String> = paths.iter().filter(|p| !present.contains(p)).collect();
        if !removed.is_empty() {
            let mut args = vec!["rm", "-f", "--ignore-unmatch", "--"];
            args.extend(removed.iter().map(|p| p.as_str()));
            self.run_git(&args)?;
        }
        Ok(())
    }

    /// `--no-verify`: hooks belong to the user's own commits, not to a mechanical rewrite.
    fn commit_with(&self, extra: &[&str]) -> Result<()> {
        let mut args = vec!["commit", "--no-verify"];
        args.extend(extra);
        self.run_git(&args).map(drop)
    }

    /// A failed rewrite must not leave the user detached in the middle of a rebase.
    /// The steps land in the journal either way; a failed one is also logged, since the
    /// user may be left detached. `rebase --abort` fails whenever no rebase was started.
    fn recover(&self, branch: &str, original: &str) {
        let _ = self.run_git(&["rebase", "--abort"]);
        for step in [
            ["checkout", "--force", branch],
            ["reset", "--hard", original],
        ] {
            if let Err(error) = self.run_git(&step) {
                tracing::error!(
                    ?error,
                    context = "putting the branch back after a failed split"
                );
            }
        }
    }

    fn rev_parse(&self, rev: &str) -> Result<String> {
        Ok(self
            .run_git_reading(&["rev-parse", "--verify", &format!("{rev}^{{commit}}")])?
            .stdout
            .trim()
            .to_owned())
    }

    fn only_parent(&self, target: &str) -> Result<String> {
        let line = self
            .run_git_reading(&["rev-list", "--parents", "-n", "1", target])?
            .stdout;
        let mut parts = line.split_whitespace().skip(1);
        let first = parts.next().ok_or_else(|| {
            GitError::InvalidState("a root commit cannot be split off".to_owned())
        })?;
        if parts.next().is_some() {
            return Err(GitError::InvalidState(
                "a merge commit cannot be split off".to_owned(),
            ));
        }
        Ok(first.to_owned())
    }

    fn current_branch(&self) -> Result<String> {
        match self.head()? {
            crate::Head::Branch { name, .. } => Ok(name),
            _ => Err(GitError::InvalidState(
                "splitting a commit needs a branch to move; HEAD is not on one".to_owned(),
            )),
        }
    }

    fn check_clean(&self) -> Result<()> {
        let status = self.status()?;
        if status.staged > 0 || status.unstaged > 0 {
            return Err(GitError::InvalidState(
                "commit or stash your changes first: splitting a commit rewrites the branch"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    /// The split has to leave something behind, or it is not a split.
    fn check_subset(&self, target: &str, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Err(GitError::InvalidState("no files chosen".to_owned()));
        }

        let touched = nul_separated(
            &self
                .run_git_reading(&[
                    "diff-tree",
                    "--no-commit-id",
                    "--name-only",
                    "-r",
                    "-z",
                    target,
                ])?
                .stdout,
        );

        if let Some(stranger) = paths.iter().find(|path| !touched.contains(path)) {
            return Err(GitError::InvalidState(format!(
                "{stranger} is not one of the files this commit changed"
            )));
        }
        if paths.len() >= touched.len() {
            return Err(GitError::InvalidState(
                "leave at least one file in the original commit".to_owned(),
            ));
        }
        Ok(())
    }
}

fn nul_separated(listing: &str) -> Vec<String> {
    listing
        .split('\0')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

impl RepoHandle {
    /// True when rewriting the commit will cost a force-push and divergence for others.
    pub fn is_published(&self, rev: &str) -> Result<bool> {
        Ok(!self.containing_remote_refs(rev)?.is_empty())
    }

    /// Every remote branch that already holds this commit. One graph walk per remote
    /// ref, so callers ask when the user acts, never on every selection.
    fn containing_remote_refs(&self, rev: &str) -> Result<Vec<String>> {
        let oid = self.rev_parse(rev)?;
        let listed = self.run_git_reading(&[
            "for-each-ref",
            "--format=%(refname:short)",
            "--contains",
            &oid,
            "refs/remotes/",
        ])?;
        Ok(listed
            .stdout
            .lines()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect())
    }
}

/// Shared branches, unless `cogit.protectedBranches` says otherwise. Rewriting a commit on
/// one of these costs everybody who has it a divergence, so surgery refuses rather than warns.
const PROTECTED: &[&str] = &["main", "master", "develop", "release/*"];

impl RepoHandle {
    /// The protected remote branches that already contain this commit, if any.
    pub fn protecting_refs(&self, rev: &str) -> Result<Vec<String>> {
        let Some(patterns) = self.protected_set() else {
            // Still resolve the revision: an unknown one is an error, not an empty list.
            self.rev_parse(rev)?;
            return Ok(Vec::new());
        };

        Ok(self
            .containing_remote_refs(rev)?
            .into_iter()
            .filter(|name| patterns.is_match(branch_of(name)))
            .collect())
    }

    /// A pattern that does not parse is skipped, not fatal: the rest still protect.
    fn protected_set(&self) -> Option<globset::GlobSet> {
        let mut builder = globset::GlobSetBuilder::new();
        let mut any = false;
        for pattern in self.protected_patterns() {
            // `literal_separator`: `release/*` is one segment, as everywhere else in git.
            match globset::GlobBuilder::new(&pattern)
                .literal_separator(true)
                .build()
            {
                Ok(glob) => {
                    builder.add(glob);
                    any = true;
                }
                Err(error) => {
                    tracing::warn!(?error, %pattern, "a protected-branch pattern was ignored");
                }
            }
        }
        if any { builder.build().ok() } else { None }
    }

    fn protected_patterns(&self) -> Vec<String> {
        match self
            .repo
            .config_snapshot()
            .string("cogit.protectedBranches")
        {
            Some(value) => value
                .to_string()
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect(),
            None => PROTECTED.iter().map(|name| (*name).to_owned()).collect(),
        }
    }
}

/// `origin/release/1.0` is matched as `release/1.0`: the remote name is not part of it.
fn branch_of(full: &str) -> &str {
    full.split_once('/').map_or(full, |(_, rest)| rest)
}
