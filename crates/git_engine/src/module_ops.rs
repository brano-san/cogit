//! Remote ▸ Submodule (#45). Every write goes through `git submodule` or `git config`.

use crate::{GitError, RepoHandle, Result};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleOp {
    Initialize,
    Synchronize,
    /// Back to the commit the parent records. git refuses to overwrite local changes.
    Reset,
    /// `submodule.<name>.active = false`: recursive commands skip it, the files stay.
    Deactivate,
    Deinit,
    /// Out of the index and `.gitmodules`; the files stay behind as an untracked folder.
    Unregister,
}

impl SubmoduleOp {
    fn needs_a_target(self) -> bool {
        !matches!(self, Self::Initialize | Self::Synchronize)
    }
}

/// A local path is how a submodule on the same disk is written; git refuses it by default.
/// Lifted only for the URL the user typed into Add: a local path in a `.gitmodules` that
/// came from elsewhere is what CVE-2022-39253 abuses, so updates keep git's ban (R-442)
/// unless the user lifted it (`submodule_update`).
const ALLOW_FILE: [&str; 2] = ["-c", "protocol.file.allow=always"];

struct Named {
    name: String,
    path: String,
}

impl RepoHandle {
    /// Empty `paths` means every submodule, for the operations that allow it.
    pub fn submodule_op(&self, op: SubmoduleOp, paths: &[String]) -> Result<()> {
        if paths.is_empty() && op.needs_a_target() {
            return Err(GitError::InvalidState(
                "choose the submodule to act on".to_owned(),
            ));
        }
        let chosen = self.chosen_modules(paths)?;
        match op {
            SubmoduleOp::Initialize => self.submodule_update(&["--init", "--recursive"], paths),
            SubmoduleOp::Synchronize => {
                self.run_on_paths(&["submodule", "sync", "--recursive"], paths)
            }
            SubmoduleOp::Reset => self.submodule_update(&["--checkout"], paths),
            SubmoduleOp::Deactivate => chosen.iter().try_for_each(|module| {
                let key = format!("submodule.{}.active", module.name);
                self.run_git(&["config", "--local", &key, "false"])
                    .map(drop)
            }),
            SubmoduleOp::Deinit => self.run_on_paths(&["submodule", "deinit"], paths),
            SubmoduleOp::Unregister => chosen.iter().try_for_each(|module| self.unregister(module)),
        }
    }

    pub fn add_submodule(&self, url: &str, path: &str, branch: Option<&str>) -> Result<()> {
        let (url, path) = (url.trim(), path.trim());
        if url.is_empty() || path.is_empty() {
            return Err(GitError::InvalidState(
                "a submodule needs a URL and a path".to_owned(),
            ));
        }
        let mut args = ALLOW_FILE.to_vec();
        args.extend(["submodule", "add"]);
        if let Some(branch) = branch.map(str::trim).filter(|branch| !branch.is_empty()) {
            args.extend(["-b", branch]);
        }
        args.extend(["--", url, path]);
        self.run_git(&args).map(drop)
    }

    /// What `.gitmodules` lists. Taken before a pull, so the ones it brings can be told apart.
    #[must_use]
    pub fn submodule_paths(&self) -> BTreeSet<String> {
        self.named_modules()
            .map(|modules| modules.into_iter().map(|module| module.path).collect())
            .unwrap_or_default()
    }

    pub fn init_submodules_added_since(&self, known: &BTreeSet<String>) -> Result<Vec<String>> {
        let added: Vec<String> = self
            .submodule_paths()
            .into_iter()
            .filter(|path| !known.contains(path))
            .collect();
        if !added.is_empty() {
            self.submodule_op(SubmoduleOp::Initialize, &added)?;
        }
        Ok(added)
    }

    /// With the repository's own `protocol.file.allow` handed on: git reads it for the clone
    /// of a submodule from the global and system files only, so a local opt-in went unheard.
    pub(crate) fn submodule_update(&self, flags: &[&str], paths: &[String]) -> Result<()> {
        let allow = self
            .repo
            .config_snapshot()
            .string("protocol.file.allow")
            .map(|value| format!("protocol.file.allow={value}"));
        let mut args = match &allow {
            Some(allow) => vec!["-c", allow.as_str()],
            None => Vec::new(),
        };
        args.extend(["submodule", "update"]);
        args.extend(flags);
        self.run_on_paths(&args, paths)
    }

    fn run_on_paths(&self, args: &[&str], paths: &[String]) -> Result<()> {
        let mut all = args.to_vec();
        all.push("--");
        all.extend(paths.iter().map(String::as_str));
        self.run_git(&all).map(drop)
    }

    fn named_modules(&self) -> Result<Vec<Named>> {
        let Some(modules) = self
            .repo
            .submodules()
            .map_err(|err| GitError::Internal(format!("cannot read .gitmodules: {err}")))?
        else {
            return Ok(Vec::new());
        };
        Ok(modules
            .filter_map(|module| {
                Some(Named {
                    name: module.name().to_string(),
                    path: module.path().ok()?.to_string(),
                })
            })
            .collect())
    }

    fn chosen_modules(&self, paths: &[String]) -> Result<Vec<Named>> {
        let modules = self.named_modules()?;
        if let Some(stranger) = paths
            .iter()
            .find(|path| !modules.iter().any(|module| &module.path == *path))
        {
            return Err(GitError::InvalidState(format!(
                "no submodule at {stranger}"
            )));
        }
        Ok(modules
            .into_iter()
            .filter(|module| paths.contains(&module.path))
            .collect())
    }

    fn unregister(&self, module: &Named) -> Result<()> {
        self.run_git(&["rm", "--cached", "-q", "--", &module.path])?;
        let section = format!("submodule.{}", module.name);
        // Emptied, not deleted, as `git rm` does: without the file gix reads HEAD's copy.
        self.run_git(&["config", "-f", ".gitmodules", "--remove-section", &section])?;
        self.run_git(&["add", "--", ".gitmodules"])?;
        if self.has_local_section("submodule", &module.name) {
            self.run_git(&["config", "--local", "--remove-section", &section])?;
        }
        Ok(())
    }

    fn has_local_section(&self, name: &str, subsection: &str) -> bool {
        let snapshot = self.repo.config_snapshot();
        snapshot
            .plumbing()
            .sections_by_name(name)
            .into_iter()
            .flatten()
            .any(|section| {
                section.meta().source == gix::config::Source::Local
                    && section.header().subsection_name() == Some(subsection.into())
            })
    }
}
