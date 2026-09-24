//! The note Cogit keeps of commits made with `--no-verify`: Git records nothing of it.

use crate::{RepoHandle, Result};
use serde::Serialize;

const BYPASS_FILE: &str = "cogit-hook-bypasses";
const SEPARATOR: char = '\u{1f}';

/// Git records nothing about `--no-verify`, so Cogit keeps its own note per clone.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Bypass {
    pub oid: String,
    pub summary: String,
    #[specta(type = specta_typescript::Number)]
    pub at: i64,
}

impl RepoHandle {
    /// Newest first; an unparsable line is skipped, never fatal.
    pub fn bypass_log(&self) -> Result<Vec<Bypass>> {
        let Ok(text) = std::fs::read_to_string(self.git_dir().join(BYPASS_FILE)) else {
            return Ok(Vec::new());
        };

        let mut entries: Vec<Bypass> = text
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(3, SEPARATOR);
                let at = parts.next()?.parse().ok()?;
                Some(Bypass {
                    at,
                    oid: parts.next()?.to_owned(),
                    summary: parts.next().unwrap_or_default().to_owned(),
                })
            })
            .collect();
        entries.reverse();
        Ok(entries)
    }

    pub(crate) fn record_bypass(&self, oid: &str, summary: &str) {
        use std::io::Write as _;

        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
        let subject = summary.lines().next().unwrap_or_default();
        let line = format!("{at}{SEPARATOR}{oid}{SEPARATOR}{subject}\n");

        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.git_dir().join(BYPASS_FILE))
            .and_then(|mut file| file.write_all(line.as_bytes()));
        if let Err(err) = written {
            tracing::error!(error = ?err, context = "failed to record a hook bypass");
        }

        // Exit zero with a line on stderr is what the Output panel marks as a warning,
        // so a bypass reads there like any other thing worth noticing (M10 T10.5).
        self.journal_entry(crate::GitOutput::record(
            self.root(),
            "hooks bypassed".to_owned(),
            Some(0),
            "",
            &format!(
                "warning: {} committed without running the hooks: {subject}",
                &oid[..7.min(oid.len())]
            ),
            0,
        ));
    }
}
