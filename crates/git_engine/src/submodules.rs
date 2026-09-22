use crate::{GitError, RepoHandle, Result};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleState {
    NotInitialised,
    InSync,
    /// Checked out on something other than the commit the parent records.
    Diverged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Submodule {
    pub name: String,
    pub path: String,
    pub url: String,
    pub recorded: String,
    pub checked_out: Option<String>,
    pub state: SubmoduleState,
    /// The branch it is on, when it is on one. A submodule is usually detached, and then
    /// the row has to name the commit instead (R-110).
    pub branch: Option<String>,
    /// First line of the commit it sits on, for the row that has no branch to show.
    pub subject: Option<String>,
    /// Whether it holds submodules of its own. Answered here so the tree can decide
    /// before drawing whether the row opens at all (doc/12-risks.md, R-148).
    pub nested: bool,
}

/// Which commit a gitlink points at on each side of a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmodulePointer {
    pub recorded: String,
    pub previous: Option<String>,
    pub checked_out: bool,
}

/// What the submodule's own repository says about itself.
struct Inside {
    oid: Option<String>,
    branch: Option<String>,
    subject: Option<String>,
}

impl RepoHandle {
    pub fn submodules(&self) -> Result<Vec<Submodule>> {
        let Some(modules) = self
            .repo
            .submodules()
            .map_err(|err| GitError::Internal(format!("cannot read .gitmodules: {err}")))?
        else {
            return Ok(Vec::new());
        };

        let mut out = Vec::new();
        for module in modules {
            let Ok(path) = module.path() else {
                continue;
            };
            let path = path.to_string();
            let recorded = module
                .head_id()
                .ok()
                .flatten()
                .map(|id| id.to_string())
                .unwrap_or_default();
            let inside = self.submodule_state(&path);
            let checked_out = inside.as_ref().and_then(|found| found.oid.clone());
            // A file test, not a second repository open: this runs per row of the tree.
            let nested =
                checked_out.is_some() && self.root().join(&path).join(".gitmodules").is_file();

            let state = match &checked_out {
                None => SubmoduleState::NotInitialised,
                Some(actual) if *actual == recorded => SubmoduleState::InSync,
                Some(_) => SubmoduleState::Diverged,
            };

            out.push(Submodule {
                name: module.name().to_string(),
                path,
                url: module
                    .url()
                    .map(|url| url.to_bstring().to_string())
                    .unwrap_or_default(),
                recorded,
                checked_out,
                state,
                branch: inside.as_ref().and_then(|found| found.branch.clone()),
                subject: inside.and_then(|found| found.subject),
                nested,
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    /// `init` also registers the submodule in `.git/config`, which a plain update skips.
    pub fn update_submodule(&self, path: &str, init: bool) -> Result<()> {
        if !self.submodules()?.iter().any(|module| module.path == path) {
            return Err(GitError::InvalidState(format!("no submodule at {path}")));
        }
        let mut args = vec!["-c", "protocol.file.allow=always", "submodule", "update"];
        if init {
            args.push("--init");
        }
        args.push("--");
        args.push(path);
        self.run_git(&args).map(drop)
    }

    /// A deinitialised submodule leaves an empty directory, and discovery walks upward
    /// from there straight into the parent. Comparing roots is what tells them apart.
    fn submodule_state(&self, path: &str) -> Option<Inside> {
        let expected = self.root().join(path);
        let inner = RepoHandle::open(&expected).ok()?;
        let same = std::fs::canonicalize(inner.root())
            .ok()
            .zip(std::fs::canonicalize(&expected).ok())
            .is_some_and(|(actual, expected)| actual == expected);
        if !same {
            return None;
        }
        let (oid, branch) = match inner.head().ok()? {
            crate::Head::Branch { oid, name } => (Some(oid), Some(name)),
            crate::Head::Detached { oid } => (Some(oid), None),
            crate::Head::Unborn { .. } => (None, None),
        };
        let subject = oid
            .as_deref()
            .and_then(|oid| inner.commit_details(oid).ok())
            .map(|details| details.summary);
        Some(Inside {
            oid,
            branch,
            subject,
        })
    }
}
