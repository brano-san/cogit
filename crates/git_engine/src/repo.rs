//! Reading a repository through `gix`.
//!
//! Reads go through `gix` rather than the CLI because spawning a process costs 40–60 ms
//! on Windows, and the commit graph needs tens of thousands of objects. Mutations take
//! the opposite route — see `doc/03-git-semantics.md`.

use crate::{GitError, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Where HEAD points.
///
/// The three variants are not decoration: a client that assumes "HEAD is a branch"
/// crashes on a freshly initialised repository and on any detached checkout (INV-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Head {
    /// The normal case: HEAD follows a branch that has at least one commit.
    Branch { name: String, oid: String },
    /// HEAD names a commit directly.
    Detached { oid: String },
    /// The branch exists in name only — no commit has been made yet.
    Unborn { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum BranchKind {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// Short form shown in the UI, such as `main` or `origin/main`.
    pub name: String,
    /// Full ref name, such as `refs/heads/main`.
    pub full_name: String,
    pub kind: BranchKind,
    /// Commit the branch points at, as hex.
    pub oid: String,
    pub is_head: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub name: String,
    pub full_name: String,
    /// Commit the tag marks, with annotated tags already peeled.
    pub oid: String,
    pub is_annotated: bool,
}

/// Open options that ignore `GIT_*` variables inherited from the environment.
///
/// By default `gix` honours `GIT_DIR`, `GIT_INDEX_FILE`, `GIT_WORK_TREE` and friends,
/// which is right for a hook but wrong for a client: launched from a Git hook or from a
/// shell where someone exported `GIT_INDEX_FILE`, Cogit would silently read a different
/// repository than the one the user picked. Only the `GIT_` prefix is denied, so the
/// user's global configuration and identity still apply. See doc/12-risks.md (R-22).
fn env_free() -> gix::open::Options {
    let mut options = gix::open::Options::default();
    options.permissions.env.git_prefix = gix::sec::Permission::Deny;
    options
}

/// An opened repository.
///
/// Holds a live `gix::Repository`, so it must not be cached across mutations performed
/// by the CLI — reopen instead. See `doc/01-architecture.md` section 6.
pub struct RepoHandle {
    /// Visible to the crate so sibling modules such as `history` can read through it.
    pub(crate) repo: gix::Repository,
    root: PathBuf,
}

impl std::fmt::Debug for RepoHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepoHandle")
            .field("root", &self.root)
            .finish()
    }
}

impl RepoHandle {
    /// Opens the repository containing `path`, searching upwards through its parents.
    ///
    /// Discovery rather than a plain open, because users point at any folder inside a
    /// project rather than at the one holding `.git`.
    ///
    /// # Errors
    /// Returns [`GitError::RepoNotFound`] when no repository encloses the path.
    pub fn open(path: &Path) -> Result<Self> {
        let repo = gix::discover_opts(path, gix::discover::upwards::Options::default(), env_free())
            .map_err(|err| GitError::RepoNotFound(format!("{}: {err}", path.display())))?;
        // A bare repository has no working tree; its git dir stands in as the root.
        let root = repo
            .workdir()
            .unwrap_or_else(|| repo.git_dir())
            .to_path_buf();
        Ok(Self { repo, root })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.repo.is_bare()
    }

    /// Resolves HEAD into one of its three possible shapes.
    ///
    /// # Errors
    /// Returns an error only if the ref store itself cannot be read.
    pub fn head(&self) -> Result<Head> {
        let head = self
            .repo
            .head()
            .map_err(|err| GitError::Internal(format!("cannot read HEAD: {err}")))?;

        // Order matters: an unborn HEAD is not detached, but it also has no id, so it
        // has to be recognised before anything tries to resolve one.
        if head.is_unborn() {
            let name = head
                .referent_name()
                .map_or_else(|| "main".to_owned(), |name| name.shorten().to_string());
            return Ok(Head::Unborn { name });
        }

        let oid = head
            .id()
            .ok_or_else(|| GitError::InvalidState("HEAD resolves to no object".to_owned()))?
            .detach()
            .to_string();

        if head.is_detached() {
            return Ok(Head::Detached { oid });
        }

        let name = head
            .referent_name()
            .ok_or_else(|| GitError::InvalidState("attached HEAD without a name".to_owned()))?
            .shorten()
            .to_string();
        Ok(Head::Branch { name, oid })
    }

    /// Lists local and remote-tracking branches, sorted by name.
    ///
    /// Sorting happens here rather than in the UI so the order does not depend on which
    /// ref backend answered — loose files and packed refs enumerate differently.
    ///
    /// # Errors
    /// Returns an error if the ref store cannot be enumerated.
    pub fn branches(&self) -> Result<Vec<Branch>> {
        let platform = self
            .repo
            .references()
            .map_err(|err| GitError::Internal(format!("cannot read references: {err}")))?;

        let head_name = self.repo.head_name().ok().flatten();
        let mut branches = Vec::new();

        let local = platform
            .local_branches()
            .map_err(|err| GitError::Internal(format!("cannot list local branches: {err}")))?;
        collect(local, BranchKind::Local, head_name.as_ref(), &mut branches);

        let remote = platform
            .remote_branches()
            .map_err(|err| GitError::Internal(format!("cannot list remote branches: {err}")))?;
        collect(
            remote,
            BranchKind::Remote,
            head_name.as_ref(),
            &mut branches,
        );

        branches.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(branches)
    }

    /// Lists tags, sorted by name, with annotated tags peeled to their commit.
    ///
    /// # Errors
    /// Returns an error if the ref store cannot be enumerated.
    pub fn tags(&self) -> Result<Vec<Tag>> {
        let platform = self
            .repo
            .references()
            .map_err(|err| GitError::Internal(format!("cannot read references: {err}")))?;
        let refs = platform
            .tags()
            .map_err(|err| GitError::Internal(format!("cannot list tags: {err}")))?;

        let mut tags = Vec::new();
        for reference in refs.flatten() {
            let full_name = reference.name().as_bstr().to_string();
            let name = reference.name().shorten().to_string();
            // An annotated tag points at a tag object, so the direct target differs from
            // the peeled one. That difference is exactly what distinguishes the two kinds.
            let direct = reference.try_id().map(|id| id.detach());
            let Ok(peeled) = reference.into_fully_peeled_id() else {
                tracing::warn!(tag = %name, "skipping a tag that does not peel to an object");
                continue;
            };
            let peeled = peeled.detach();
            tags.push(Tag {
                name,
                full_name,
                oid: peeled.to_string(),
                is_annotated: direct != Some(peeled),
            });
        }
        tags.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tags)
    }
}

/// Turns an iterator of references into branches, skipping what cannot be a branch.
///
/// Broken or symbolic refs are skipped rather than failing the whole listing: one
/// unreadable ref must not hide every other branch from the user.
fn collect<'a>(
    references: impl Iterator<
        Item = std::result::Result<gix::Reference<'a>, Box<dyn std::error::Error + Send + Sync>>,
    >,
    kind: BranchKind,
    head_name: Option<&gix::refs::FullName>,
    out: &mut Vec<Branch>,
) {
    for reference in references {
        let Ok(reference) = reference else {
            tracing::warn!(?kind, "skipping an unreadable reference");
            continue;
        };
        // `origin/HEAD` is symbolic and has no object of its own.
        let Some(id) = reference.try_id() else {
            continue;
        };
        let full_name = reference.name();
        out.push(Branch {
            name: full_name.shorten().to_string(),
            full_name: full_name.as_bstr().to_string(),
            kind,
            oid: id.detach().to_string(),
            is_head: head_name.is_some_and(|head| head.as_ref() == full_name),
        });
    }
}
