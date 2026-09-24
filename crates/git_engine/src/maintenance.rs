use crate::RepoHandle;
use crate::runner::{GitOutput, elapsed_ms, redact_command};

impl RepoHandle {
    /// The `maintenance run --auto` git starts at the end of a commit, started after it
    /// instead: git waits for that child before it returns, 44 ms of every commit
    /// (doc/12-risks.md, R-314). The commit runs with `maintenance.auto=false`.
    pub(crate) fn maintain_after_commit(&self) {
        let config = self.repo.config_snapshot();
        if config.boolean("maintenance.auto") == Some(false) {
            return;
        }
        // As git decides it for the child it would have started (`run_auto_maintenance`).
        let detach = config
            .boolean("maintenance.autoDetach")
            .or_else(|| config.boolean("gc.autoDetach"))
            .unwrap_or(true);
        let args = [
            "maintenance",
            "run",
            "--auto",
            "--no-quiet",
            if detach { "--detach" } else { "--no-detach" },
        ];
        let mut command = self.base_git(&args);
        let root = self.root().to_path_buf();
        let journal = self.journal().cloned();

        let spawned = std::thread::Builder::new()
            .name("git maintenance".to_owned())
            .spawn(move || {
                let started = std::time::Instant::now();
                let output = match crate::children::output(&mut command) {
                    Ok(output) => output,
                    Err(err) => {
                        tracing::error!(error = ?err, context = "auto maintenance after a commit");
                        return;
                    }
                };
                // Mostly there is nothing to do and nothing said. When there is, the journal
                // gets it, as it got it inside the commit's own stderr before.
                if output.status.success() && output.stdout.is_empty() && output.stderr.is_empty() {
                    return;
                }
                let record = GitOutput::record(
                    &root,
                    redact_command(&args),
                    output.status.code(),
                    &String::from_utf8_lossy(&output.stdout),
                    &String::from_utf8_lossy(&output.stderr),
                    elapsed_ms(started),
                );
                if let Some(sink) = journal {
                    sink(record);
                }
            });
        if let Err(err) = spawned {
            tracing::error!(error = ?err, context = "auto maintenance thread");
        }
    }
}
