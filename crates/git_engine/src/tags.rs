use crate::{GitError, RepoHandle, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TagRequest {
    pub name: String,
    /// `None` means HEAD.
    pub target: Option<String>,
    /// A message makes the tag annotated, which is what a release wants.
    pub message: Option<String>,
    pub force: bool,
}

impl RepoHandle {
    pub fn create_tag(&self, request: &TagRequest) -> Result<()> {
        self.write_tag(request, false)
    }

    /// `verbatim` keeps the message as given: a copy of an existing one must not lose its
    /// `#` lines or trailing spaces to git's default cleanup.
    fn write_tag(&self, request: &TagRequest, verbatim: bool) -> Result<()> {
        let name = require(&request.name)?;

        let mut args = vec!["tag"];
        if verbatim {
            args.push("--cleanup=verbatim");
        }
        if request.force {
            args.push("--force");
        }
        if let Some(message) = &request.message {
            args.push("--annotate");
            args.push("--message");
            args.push(message);
        }
        args.push(name);
        if let Some(target) = &request.target {
            args.push(target);
        }
        self.run_git(&args).map(drop)
    }

    pub fn delete_tag(&self, name: &str) -> Result<()> {
        let name = require(name)?;
        self.run_git(&["tag", "--delete", name]).map(drop)
    }

    /// Asked once on submit. A refusal is the answer, not a failed command, so it stays out
    /// of the journal: a journalled failure would open the Git error window over the dialog.
    pub fn tag_name_problem(&self, name: &str) -> Result<Option<String>> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(Some("Enter a name.".to_owned()));
        }
        let full = format!("refs/tags/{name}");
        let checked = crate::children::output(&mut self.base_git(&["check-ref-format", &full]))?;
        if !checked.status.success() {
            return Ok(Some(format!("'{name}' is not a valid tag name.")));
        }
        if self.repo.find_reference(full.as_str()).is_ok() {
            return Ok(Some(format!("A tag named '{name}' already exists.")));
        }
        Ok(None)
    }

    pub fn tag_message(&self, name: &str) -> Result<Option<String>> {
        let full = format!("refs/tags/{}", require(name)?);
        let reference = self
            .repo
            .find_reference(full.as_str())
            .map_err(|err| GitError::InvalidState(format!("no tag {name}: {err}")))?;
        let Some(id) = reference.try_id() else {
            return Ok(None);
        };
        let object = id
            .object()
            .map_err(|err| GitError::Internal(format!("cannot read tag {name}: {err}")))?;
        if object.kind != gix::object::Kind::Tag {
            return Ok(None);
        }
        let tag = object.into_tag();
        let decoded = tag
            .decode()
            .map_err(|err| GitError::Internal(format!("cannot decode tag {name}: {err}")))?;
        Ok(Some(decoded.message.to_string().trim_end().to_owned()))
    }

    /// The new tag first, so a refusal leaves the old one standing (R-251).
    pub fn rename_tag(&self, from: &str, to: &str) -> Result<()> {
        let from = require(from)?;
        let to = require(to)?;
        let message = self.tag_message(from)?;
        self.write_tag(
            &TagRequest {
                name: to.to_owned(),
                target: Some(format!("refs/tags/{from}^{{}}")),
                message,
                force: false,
            },
            true,
        )?;
        self.delete_tag(from)
    }
}

fn require(name: &str) -> Result<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(GitError::InvalidState("an empty tag name".to_owned()));
    }
    Ok(trimmed)
}
