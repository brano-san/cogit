use crate::{GitError, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Assuming "HEAD is a branch" crashes on an unborn or detached checkout (INV-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Head {
    Branch { name: String, oid: String },
    Detached { oid: String },
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
    pub name: String,
    pub full_name: String,
    pub kind: BranchKind,
    pub oid: String,
    pub is_head: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub name: String,
    pub full_name: String,
    pub oid: String,
    pub is_annotated: bool,
}

/// Ignores inherited `GIT_*` variables: `gix` honours `GIT_INDEX_FILE` and friends,
/// which is right for a hook and wrong for a client (doc/12-risks.md, R-22).
fn env_free() -> gix::open::Options {
    let mut options = gix::open::Options::default();
    options.permissions.env.git_prefix = gix::sec::Permission::Deny;
    options
}

pub struct RepoHandle {
    pub(crate) repo: gix::Repository,
    root: PathBuf,
    journal: Option<crate::CommandSink>,
}

impl std::fmt::Debug for RepoHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepoHandle")
            .field("root", &self.root)
            .finish()
    }
}

impl RepoHandle {
    pub fn open(path: &Path) -> Result<Self> {
        let repo = gix::discover_opts(path, gix::discover::upwards::Options::default(), env_free())
            .map_err(|err| GitError::RepoNotFound(format!("{}: {err}", path.display())))?;
        let root = repo
            .workdir()
            .unwrap_or_else(|| repo.git_dir())
            .to_path_buf();
        Ok(Self {
            repo,
            root,
            journal: None,
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn with_journal(mut self, sink: crate::CommandSink) -> Self {
        self.journal = Some(sink);
        self
    }

    pub(crate) fn journal(&self) -> Option<&crate::CommandSink> {
        self.journal.as_ref()
    }

    #[must_use]
    pub fn git_dir(&self) -> &Path {
        self.repo.git_dir()
    }

    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.repo.is_bare()
    }

    pub fn head(&self) -> Result<Head> {
        let head = self
            .repo
            .head()
            .map_err(|err| GitError::Internal(format!("cannot read HEAD: {err}")))?;

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
