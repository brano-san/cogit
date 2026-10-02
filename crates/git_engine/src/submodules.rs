use crate::{GitError, RepoHandle, RepoState, Result};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleState {
    NotInitialised,
    InSync,
    /// New commits on top of the recorded one: commit the pointer in the parent.
    Ahead,
    /// On an ancestor of the recorded commit: `git submodule update`.
    Behind,
    /// Neither contains the other; only a person can decide which side wins.
    Diverged,
    /// The recorded commit is not in the submodule, so where it stands cannot be told.
    Unknown,
    /// Checked out, and not looked into: the outline of a repository not on screen (R-352).
    Unread,
    /// Listed in `.gitmodules`, with no gitlink in HEAD or the index to compare with.
    Unrecorded,
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
    /// Commits on each side of the merge base, for the tooltip; zero unless ahead,
    /// behind or diverged.
    pub ahead: u32,
    pub behind: u32,
    /// What the submodule's own repository is in the middle of; `None` until it is checked out.
    pub repo_state: Option<RepoState>,
    /// `submodule.<name>.update` as configured: `rebase`, `merge`, `none` or `!command`;
    /// `None` for the default checkout.
    pub update: Option<String>,
    /// A `./` or `../` URL resolved against the parent's remote, as git clones it.
    pub resolved_url: Option<String>,
}

/// git's `resolve_relative_url`: each `../` drops one component of the parent's remote
/// URL (or of its folder, without a remote).
#[must_use]
pub fn resolve_relative_url(url: &str, base: &str) -> Option<String> {
    if !(url.starts_with("./") || url.starts_with("../")) {
        return None;
    }
    let mut base = base.trim_end_matches('/').to_owned();
    if let Some(stripped) = base.strip_suffix("/.git") {
        base = stripped.to_owned();
    }
    let mut rest = url;
    loop {
        if let Some(next) = rest.strip_prefix("./") {
            rest = next;
        } else if let Some(next) = rest.strip_prefix("../") {
            rest = next;
            let cut = base.rfind(['/', ':'])?;
            base.truncate(cut + usize::from(base.as_bytes()[cut] == b':'));
        } else {
            break;
        }
    }
    let joint = if base.ends_with(':') { "" } else { "/" };
    Some(format!("{base}{joint}{rest}"))
}

fn update_mode(module: &gix::Submodule<'_>) -> Option<String> {
    use gix::submodule::config::Update;
    match module.update().ok().flatten()? {
        Update::Checkout => None,
        Update::Rebase => Some("rebase".to_owned()),
        Update::Merge => Some("merge".to_owned()),
        Update::None => Some("none".to_owned()),
        Update::Command(command) => Some(format!("!{command}")),
    }
}

/// Which commit a gitlink points at on each side of a diff; `recorded` is `None` on the
/// side that removed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmodulePointer {
    pub recorded: Option<String>,
    pub previous: Option<String>,
    pub checked_out: bool,
    /// The index has the gitlink, which `git submodule update --init` needs.
    pub in_index: bool,
}

/// What the submodule's own repository says about itself.
struct Inside {
    handle: RepoHandle,
    oid: Option<String>,
    branch: Option<String>,
    subject: Option<String>,
    state: Option<RepoState>,
}

impl Inside {
    /// Where the checkout stands against the recorded commit. Asked only when the two
    /// differ, which in a healthy tree is one submodule in ten, so it is not deferred.
    fn place(&self, actual: &str, recorded: &str) -> (SubmoduleState, u32, u32) {
        let (Ok(actual), Ok(recorded)) = (
            gix::ObjectId::from_hex(actual.as_bytes()),
            gix::ObjectId::from_hex(recorded.as_bytes()),
        ) else {
            return (SubmoduleState::Unknown, 0, 0);
        };
        if self.handle.repo.find_object(recorded).is_err() {
            return (SubmoduleState::Unknown, 0, 0);
        }
        match self.handle.count_divergence(actual, recorded) {
            Some((0, 0)) => (SubmoduleState::InSync, 0, 0),
            Some((ahead, 0)) => (SubmoduleState::Ahead, ahead, 0),
            Some((0, behind)) => (SubmoduleState::Behind, 0, behind),
            Some((ahead, behind)) => (SubmoduleState::Diverged, ahead, behind),
            // Unrelated histories are counted; this is a history that could not be walked.
            None => (SubmoduleState::Unknown, 0, 0),
        }
    }
}

/// HEAD's gitlink, else the index's: a submodule just added is recorded only there until
/// the commit. Empty when neither side has one.
fn recorded(module: &gix::Submodule<'_>) -> String {
    module
        .head_id()
        .ok()
        .flatten()
        .or_else(|| module.index_id().ok().flatten())
        .map(|id| id.to_string())
        .unwrap_or_default()
}

fn url(module: &gix::Submodule<'_>) -> String {
    module
        .url()
        .map(|url| url.to_bstring().to_string())
        .unwrap_or_default()
}

impl RepoHandle {
    fn modules(&self) -> Result<Vec<gix::Submodule<'_>>> {
        let modules = self
            .repo
            .submodules()
            .map_err(|err| GitError::Internal(format!("cannot read .gitmodules: {err}")))?;
        Ok(modules.map(Iterator::collect).unwrap_or_default())
    }

    /// `.gitmodules` and the gitlinks of HEAD, plus two file tests per row; no submodule is
    /// opened, so no status and no history. For the trees of repositories not on screen.
    pub fn submodule_outline(&self) -> Result<Vec<Submodule>> {
        let base = self.relative_url_base();
        let mut out = Vec::new();
        for module in self.modules()? {
            let Ok(path) = module.path() else {
                continue;
            };
            let path = path.to_string();
            let dir = self.root().join(&path);
            let initialised = dir.join(".git").exists();
            out.push(Submodule {
                name: module.name().to_string(),
                update: update_mode(&module),
                resolved_url: resolve_relative_url(&url(&module), &base),
                url: url(&module),
                recorded: recorded(&module),
                checked_out: None,
                state: if initialised {
                    SubmoduleState::Unread
                } else {
                    SubmoduleState::NotInitialised
                },
                branch: None,
                subject: None,
                nested: initialised && dir.join(".gitmodules").is_file(),
                ahead: 0,
                behind: 0,
                repo_state: None,
                path,
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    pub fn submodules(&self) -> Result<Vec<Submodule>> {
        let base = self.relative_url_base();
        let mut out = Vec::new();
        for module in self.modules()? {
            let Ok(path) = module.path() else {
                continue;
            };
            let path = path.to_string();
            let recorded = recorded(&module);
            let inside = self.submodule_state(&path);
            let checked_out = inside.as_ref().and_then(|found| found.oid.clone());
            // A file test, not a second repository open: this runs per row of the tree.
            let nested =
                checked_out.is_some() && self.root().join(&path).join(".gitmodules").is_file();

            let (state, ahead, behind) = match (&checked_out, &inside) {
                (None, _) | (_, None) => (SubmoduleState::NotInitialised, 0, 0),
                _ if recorded.is_empty() => (SubmoduleState::Unrecorded, 0, 0),
                (Some(actual), _) if *actual == recorded => (SubmoduleState::InSync, 0, 0),
                (Some(actual), Some(found)) => found.place(actual, &recorded),
            };

            out.push(Submodule {
                name: module.name().to_string(),
                path,
                update: update_mode(&module),
                resolved_url: resolve_relative_url(&url(&module), &base),
                url: url(&module),
                recorded,
                checked_out,
                state,
                branch: inside.as_ref().and_then(|found| found.branch.clone()),
                repo_state: inside.as_ref().and_then(|found| found.state.clone()),
                subject: inside.and_then(|found| found.subject),
                nested,
                ahead,
                behind,
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    /// The URL of the branch's remote, else `origin`, else the repository's folder.
    fn relative_url_base(&self) -> String {
        let remote = self
            .repo
            .head_name()
            .ok()
            .flatten()
            .and_then(|head| {
                self.repo
                    .branch_remote_name(head.shorten(), gix::remote::Direction::Fetch)
                    .map(|name| name.as_bstr().to_string())
            })
            .unwrap_or_else(|| "origin".to_owned());
        self.repo
            .config_snapshot()
            .string(format!("remote.{remote}.url").as_str())
            .map_or_else(|| crate::slash_path(self.root()), |url| url.to_string())
    }

    /// `init` also registers the submodule in `.git/config`, which a plain update skips.
    pub fn update_submodule(&self, path: &str, init: bool) -> Result<()> {
        if !self.submodules()?.iter().any(|module| module.path == path) {
            return Err(GitError::InvalidState(format!("no submodule at {path}")));
        }
        let flags: &[&str] = if init { &["--init"] } else { &[] };
        self.submodule_update(flags, &[path.to_owned()])
    }

    /// A deinitialised submodule leaves an empty directory; `open_exact` refuses it rather
    /// than discovering its way up into the parent.
    fn submodule_state(&self, path: &str) -> Option<Inside> {
        let inner = RepoHandle::open_exact(&self.root().join(path)).ok()?;
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
            state: inner.state().ok(),
            handle: inner,
            oid,
            branch,
            subject,
        })
    }
}

#[cfg(test)]
mod relative_url_tests {
    use super::resolve_relative_url;

    #[test]
    fn relative_urls_resolve_against_the_parent_remote() {
        let base = "https://host/team/app.git";
        assert_eq!(
            resolve_relative_url("../lib.git", base).as_deref(),
            Some("https://host/team/lib.git")
        );
        assert_eq!(
            resolve_relative_url("./sub", base).as_deref(),
            Some("https://host/team/app.git/sub")
        );
        assert_eq!(
            resolve_relative_url("../lib", "git@host:team/app").as_deref(),
            Some("git@host:team/lib")
        );
        assert_eq!(resolve_relative_url("https://x/y", base), None);
    }
}
