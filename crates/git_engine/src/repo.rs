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
    /// Short name of the tracking branch, e.g. `origin/main`.
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub name: String,
    pub full_name: String,
    /// Peeled through every tag object, so the label sits on the commit (R-157).
    pub oid: String,
    pub is_annotated: bool,
    /// False for a tag on a tree or a blob: it cannot start a walk, so its box is off.
    pub points_to_commit: bool,
}

/// Ignores inherited `GIT_*` variables: `gix` honours `GIT_INDEX_FILE` and friends,
/// which is right for a hook and wrong for a client (doc/12-risks.md, R-22).
/// Enough for the topological window of the graph walk several times over.
const OBJECT_CACHE_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn env_free() -> gix::open::Options {
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
        Ok(Self::from_repo(repo))
    }

    pub(crate) fn from_repo(mut repo: gix::Repository) -> Self {
        // The graph walk reads a commit for its parents and again, up to 2 048 commits
        // later, for its message; without a cache that second read unpacks it again.
        repo.object_cache_size_if_unset(OBJECT_CACHE_BYTES);
        let root = repo
            .workdir()
            .unwrap_or_else(|| repo.git_dir())
            .to_path_buf();
        Self {
            repo,
            root,
            journal: None,
        }
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

        for branch in &mut branches {
            if branch.kind == BranchKind::Local {
                self.fill_upstream(branch);
            }
        }

        branches.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(branches)
    }

    fn fill_upstream(&self, branch: &mut Branch) {
        let Ok(full) = gix::refs::FullName::try_from(branch.full_name.as_str()) else {
            return;
        };
        let Some(Ok(tracking)) = self
            .repo
            .branch_remote_tracking_ref_name(full.as_ref(), gix::remote::Direction::Fetch)
        else {
            return;
        };
        branch.upstream = Some(tracking.shorten().to_string());

        let Ok(mut reference) = self.repo.find_reference(tracking.as_ref()) else {
            return;
        };
        let Ok(upstream_id) = reference.peel_to_id() else {
            return;
        };
        let Ok(local_id) = gix::ObjectId::from_hex(branch.oid.as_bytes()) else {
            return;
        };

        if let Some((ahead, behind)) = self.count_divergence(local_id, upstream_id.detach()) {
            branch.ahead = ahead;
            branch.behind = behind;
        }
    }

    /// Commits on each side of the merge base. Through `gix`: a `rev-list` per branch
    /// would mean one process per row in a 500-branch repository.
    pub(crate) fn count_divergence(
        &self,
        local: gix::ObjectId,
        upstream: gix::ObjectId,
    ) -> Option<(u32, u32)> {
        if local == upstream {
            return Some((0, 0));
        }
        let base = self.repo.merge_base(local, upstream).ok()?.detach();
        Some((
            self.count_between(local, base)?,
            self.count_between(upstream, base)?,
        ))
    }

    fn count_between(&self, tip: gix::ObjectId, base: gix::ObjectId) -> Option<u32> {
        if tip == base {
            return Some(0);
        }
        let walk = self
            .repo
            .rev_walk(Some(tip))
            .with_hidden(Some(base))
            .all()
            .ok()?;
        u32::try_from(walk.filter_map(std::result::Result::ok).count()).ok()
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
                points_to_commit: self
                    .repo
                    .find_header(peeled)
                    .is_ok_and(|header| header.kind() == gix::object::Kind::Commit),
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
            upstream: None,
            ahead: 0,
            behind: 0,
        });
    }
}
