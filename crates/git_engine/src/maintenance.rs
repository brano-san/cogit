use crate::RepoHandle;
use crate::runner::{GitOutput, elapsed_ms, redact_command};

impl RepoHandle {
    /// Whether git would have run `maintenance run --auto` at the end of a commit.
    #[must_use]
    pub fn wants_maintenance(&self) -> bool {
        self.repo.config_snapshot().boolean("maintenance.auto") != Some(false)
    }

    /// The `maintenance run --auto` git starts at the end of a commit, run after it
    /// instead: git waits for that child before it returns, 44 ms of every commit
    /// (doc/12-risks.md, R-314). The commit runs with `maintenance.auto=false`, and the
    /// caller runs this in a turn of its own in the repository's queue (R-444).
    pub fn maintain_after_commit(&self) {
        if !self.wants_maintenance() {
            return;
        }
        let config = self.repo.config_snapshot();
        // As git decides it for the child it would have started (`run_auto_maintenance`).
        let detach = config
            .boolean("maintenance.autoDetach")
            .or_else(|| config.boolean("gc.autoDetach"))
            .unwrap_or(true);
        let args = maintenance_args(detach, git_release());
        let mut command = self.base_git(&args);

        let started = std::time::Instant::now();
        let output = match crate::children::output(&mut command) {
            Ok(output) => output,
            Err(err) => {
                tracing::error!(error = ?err, context = "auto maintenance after a commit");
                return;
            }
        };
        // Mostly there is nothing to do and nothing said. When there is, the journal gets
        // it, as it got it inside the commit's own stderr before.
        if output.status.success() && output.stdout.is_empty() && output.stderr.is_empty() {
            return;
        }
        let record = GitOutput::record(
            self.root(),
            redact_command(&args),
            output.status.code(),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
            elapsed_ms(started),
        );
        self.journal_entry(record);
    }
}

/// The running git as (major, minor), asked once per process.
fn git_release() -> Option<(u32, u32)> {
    static RELEASE: std::sync::OnceLock<Option<(u32, u32)>> = std::sync::OnceLock::new();
    *RELEASE.get_or_init(|| match crate::runner::git_version() {
        Ok(line) => release_of(&line),
        Err(err) => {
            tracing::error!(error = ?err, context = "git version for auto maintenance");
            None
        }
    })
}

/// `git version 2.51.0.windows.1` → (2, 51).
fn release_of(line: &str) -> Option<(u32, u32)> {
    let mut numbers = line
        .strip_prefix("git version ")?
        .split(|c: char| !c.is_ascii_digit())
        .map(str::parse::<u32>);
    Some((numbers.next()?.ok()?, numbers.next()?.ok()?))
}

/// `--[no-]detach` came with git 2.47, and an older git exits 129 on it. Without the flag
/// an older git decides by `gc.autoDetach` itself, as it did inside the commit.
fn maintenance_args(detach: bool, release: Option<(u32, u32)>) -> Vec<&'static str> {
    let mut args = vec!["maintenance", "run", "--auto", "--no-quiet"];
    if release.is_some_and(|release| release >= (2, 47)) {
        args.push(if detach { "--detach" } else { "--no-detach" });
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ubuntu 24.04 ships git 2.43: `--no-detach` was "unknown option", exit 129, a failed
    // command in the journal after every commit and no maintenance at all.
    #[test]
    fn a_git_older_than_2_47_is_not_given_the_detach_flag() {
        assert_eq!(
            maintenance_args(true, Some((2, 43))),
            ["maintenance", "run", "--auto", "--no-quiet"]
        );
        assert_eq!(
            maintenance_args(false, Some((2, 46))),
            ["maintenance", "run", "--auto", "--no-quiet"]
        );
    }

    #[test]
    fn git_2_47_and_later_are_told_whether_to_detach() {
        assert_eq!(
            maintenance_args(true, Some((2, 47))),
            ["maintenance", "run", "--auto", "--no-quiet", "--detach"]
        );
        assert_eq!(
            maintenance_args(false, Some((3, 0))),
            ["maintenance", "run", "--auto", "--no-quiet", "--no-detach"]
        );
    }

    #[test]
    fn an_unknown_version_gets_the_form_every_git_understands() {
        assert_eq!(
            maintenance_args(true, None),
            ["maintenance", "run", "--auto", "--no-quiet"]
        );
    }

    #[test]
    fn a_version_line_is_read_as_major_and_minor() {
        assert_eq!(release_of("git version 2.51.0.windows.1"), Some((2, 51)));
        assert_eq!(release_of("git version 2.43.0"), Some((2, 43)));
        assert_eq!(
            release_of("git version 2.39.5 (Apple Git-154)"),
            Some((2, 39))
        );
        assert_eq!(release_of("something else"), None);
    }
}
