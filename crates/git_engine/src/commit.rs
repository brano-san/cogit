use crate::{GitError, RepoHandle, Result};
use serde::Serialize;
use std::ops::ControlFlow;

const SECONDS_PER_MINUTE: i32 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    pub name: String,
    pub email: String,
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
    pub tz_offset_minutes: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetails {
    pub oid: String,
    pub parents: Vec<String>,
    pub summary: String,
    pub body: String,
    pub author: Signature,
    pub committer: Signature,
    pub notes: Vec<crate::CommitNote>,
    /// `Key: value` lines closing the message, `Co-authored-by` and the like.
    pub trailers: Vec<Trailer>,
    /// Carries a `gpgsig` header; `commit_signature` says whether it verifies.
    pub signed: bool,
    /// The encoding the message was decoded from when it was not UTF-8.
    pub encoding: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Trailer {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SignatureCheck {
    /// `%G?`: G good, B bad, U good but untrusted, X/Y expired, R revoked, E cannot check.
    pub status: String,
    pub signer: String,
    pub key: String,
    /// What gpg or ssh-keygen said, unaltered.
    pub raw: String,
}

/// The last paragraph, when every line in it is a trailer or continues one.
pub(crate) fn parse_trailers(body: &str) -> Vec<Trailer> {
    let Some(last) = body.trim_end().rsplit("\n\n").next() else {
        return Vec::new();
    };
    let mut trailers: Vec<Trailer> = Vec::new();
    for line in last.lines() {
        if line.starts_with([' ', '\t']) {
            match trailers.last_mut() {
                Some(trailer) => {
                    trailer.value.push(' ');
                    trailer.value.push_str(line.trim());
                    continue;
                }
                None => return Vec::new(),
            }
        }
        let Some((key, value)) = line.split_once(':') else {
            return Vec::new();
        };
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Vec::new();
        }
        trailers.push(Trailer {
            key: key.to_owned(),
            value: value.trim().to_owned(),
        });
    }
    trailers
}

/// The `encoding` header first, then `i18n.commitEncoding`; UTF-8 bytes stay as they are.
pub(crate) fn decode_message(
    raw: &[u8],
    header: Option<&str>,
    configured: Option<&str>,
) -> (String, Option<String>) {
    let named = header
        .or(configured)
        .and_then(|label| encoding_rs::Encoding::for_label(label.trim().as_bytes()))
        .filter(|encoding| *encoding != encoding_rs::UTF_8);
    let encoding = match (named, std::str::from_utf8(raw)) {
        (None, Ok(text)) => return (text.to_owned(), None),
        (Some(encoding), _) => encoding,
        (None, Err(_)) => encoding_rs::WINDOWS_1252,
    };
    let (text, _) = encoding.decode_without_bom_handling(raw);
    (text.into_owned(), Some(encoding.name().to_owned()))
}

fn split_message(text: &str) -> (String, String) {
    let text = text.trim_start_matches('\n');
    let (head, body) = text.split_once("\n\n").unwrap_or((text, ""));
    let summary = head.lines().map(str::trim).collect::<Vec<_>>().join(" ");
    (summary, body.trim_end().to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Conflicted,
    /// Tracked and identical to the index; only listed when the panel asks for it.
    Unchanged,
    Ignored,
    AssumeUnchanged,
    Skipped,
    /// Skip-worktree under `core.sparseCheckout` and absent on disk: hidden, not deleted.
    Sparse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FileMode {
    Plain,
    Executable,
    Symlink,
    Submodule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    /// What the entry is. A changed submodule is drawn with the submodule icon, the same
    /// one the Repositories panel uses, not as a file (doc/12-risks.md, R-143).
    pub mode: FileMode,
    /// The new mode, only when it differs from the old one.
    pub mode_change: Option<FileMode>,
    /// Percent, only for a rename or a copy.
    pub similarity: Option<u32>,
    /// Only on a worktree row of a submodule: what differs inside it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub submodule: Option<SubmoduleChange>,
}

/// What a submodule row stands for. `git add` records only the commit (`new_commits`);
/// `modified` and `untracked` are edits inside the submodule that no parent commit can hold.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleChange {
    pub new_commits: bool,
    pub modified: bool,
    pub untracked: bool,
}

/// Git's own default: below this the two sides are an add and a delete, not a rename.
pub const DEFAULT_SIMILARITY: u32 = 50;

pub(crate) fn mode_of_entry(mode: gix::object::tree::EntryMode) -> FileMode {
    if mode.is_link() {
        FileMode::Symlink
    } else if mode.is_commit() {
        FileMode::Submodule
    } else if mode.is_executable() {
        FileMode::Executable
    } else {
        FileMode::Plain
    }
}

impl RepoHandle {
    /// Author and committer as `.mailmap` names them, like `git log --use-mailmap`.
    pub fn commit_details(&self, rev: &str) -> Result<CommitDetails> {
        self.commit_details_with(rev, &self.mailmap())
    }

    /// For a loop over many commits: the mailmap is checked once, not per commit.
    pub(crate) fn commit_details_with(
        &self,
        rev: &str,
        mailmap: &crate::Mailmap,
    ) -> Result<CommitDetails> {
        let commit = self.find_commit(rev)?;

        let decoded = commit
            .decode()
            .map_err(|err| GitError::Internal(format!("cannot read commit: {err}")))?;
        let signed = decoded
            .extra_headers()
            .find("gpgsig")
            .or_else(|| decoded.extra_headers().find("gpgsig-sha256"))
            .is_some();
        let header = decoded.encoding.map(|name| name.to_string());
        let configured = self
            .repo
            .config_snapshot()
            .string("i18n.commitEncoding")
            .map(|value| value.to_string());
        let (text, encoding) =
            decode_message(decoded.message, header.as_deref(), configured.as_deref());
        let (summary, body) = split_message(&text);
        let author = commit
            .author()
            .map_err(|err| GitError::Internal(format!("cannot read commit author: {err}")))?;
        let committer = commit
            .committer()
            .map_err(|err| GitError::Internal(format!("cannot read committer: {err}")))?;

        Ok(CommitDetails {
            oid: commit.id().to_string(),
            parents: commit.parent_ids().map(|id| id.to_string()).collect(),
            trailers: parse_trailers(&body),
            summary,
            body,
            author: signature(author, mailmap),
            committer: signature(committer, mailmap),
            notes: self.notes_of(commit.id),
            signed,
            encoding,
        })
    }

    /// Verified by gpg or ssh-keygen as `gpg.format` says, through git's own `%G` fields.
    pub fn commit_signature(&self, rev: &str) -> Result<SignatureCheck> {
        let text = self.read_git(&[
            "log",
            "-1",
            "--no-walk",
            "--format=%G?%x00%GS%x00%GK%x00%GG",
            rev,
            "--",
        ])?;
        let mut fields = text.splitn(4, '\0');
        let mut next = || fields.next().unwrap_or_default().trim().to_owned();
        Ok(SignatureCheck {
            status: next(),
            signer: next(),
            key: next(),
            raw: next(),
        })
    }

    pub fn commit_files(&self, rev: &str) -> Result<Vec<FileEntry>> {
        self.commit_files_with(rev, DEFAULT_SIMILARITY)
    }

    /// `similarity` is the rename threshold in percent, as `git diff -M<n>%`.
    pub fn commit_files_with(&self, rev: &str, similarity: u32) -> Result<Vec<FileEntry>> {
        let commit = self.find_commit(rev)?;
        let tree = commit
            .tree()
            .map_err(|err| GitError::Internal(format!("cannot read commit tree: {err}")))?;

        let parent_tree = match commit.parent_ids().next() {
            Some(id) => self
                .repo
                .find_commit(id.detach())
                .map_err(|err| GitError::Internal(format!("cannot read parent commit: {err}")))?
                .tree()
                .map_err(|err| GitError::Internal(format!("cannot read parent tree: {err}")))?,
            None => self.repo.empty_tree(),
        };

        self.files_between_trees(&parent_tree, &tree, similarity)
    }

    /// The same tree diff the commit view uses, reachable from anything holding two trees —
    /// a stash keeps three of them (T5.2).
    pub(crate) fn files_between_trees(
        &self,
        before: &gix::Tree<'_>,
        after: &gix::Tree<'_>,
        similarity: u32,
    ) -> Result<Vec<FileEntry>> {
        let mut files = Vec::new();
        before
            .changes()
            .map_err(|err| GitError::Internal(format!("cannot start a tree diff: {err}")))?
            .options(|options| {
                options.track_path();
                options.track_rewrites(Some(gix::diff::Rewrites {
                    copies: Some(gix::diff::rewrites::Copies::default()),
                    percentage: Some(similarity as f32 / 100.0),
                    limit: 0,
                    track_empty: false,
                }));
            })
            .for_each_to_obtain_tree(after, |change| {
                if let Some(entry) = to_entry(&change) {
                    files.push(entry);
                }
                Ok::<_, std::convert::Infallible>(ControlFlow::Continue(()))
            })
            .map_err(|err| GitError::Internal(format!("tree diff failed: {err}")))?;

        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    pub(crate) fn tree_of(&self, oid: gix::ObjectId) -> Result<gix::Tree<'_>> {
        self.repo
            .find_commit(oid)
            .map_err(|err| GitError::Internal(format!("cannot read commit {oid}: {err}")))?
            .tree()
            .map_err(|err| GitError::Internal(format!("cannot read tree of {oid}: {err}")))
    }

    pub(crate) fn find_commit(&self, rev: &str) -> Result<gix::Commit<'_>> {
        let id = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?;
        id.object()
            .map_err(|err| GitError::Internal(format!("cannot read {rev}: {err}")))?
            .try_into_commit()
            .map_err(|err| GitError::InvalidState(format!("{rev} is not a commit: {err}")))
    }
}

fn signature(sig: gix::actor::SignatureRef<'_>, mailmap: &crate::Mailmap) -> Signature {
    let time = sig.time().unwrap_or_default();
    let (mut name, mut email) = (sig.name.to_string(), sig.email.to_string());
    mailmap.apply(&mut name, &mut email);
    Signature {
        name,
        email,
        timestamp: time.seconds,
        tz_offset_minutes: time.offset / SECONDS_PER_MINUTE,
    }
}

fn to_entry(change: &gix::object::tree::diff::Change<'_, '_, '_>) -> Option<FileEntry> {
    use gix::object::tree::diff::Change;

    let (path, old_path, status, mode, mode_change, similarity) = match change {
        Change::Addition {
            location,
            entry_mode,
            ..
        } => {
            if entry_mode.is_tree() {
                return None;
            }
            (
                location,
                None,
                FileStatus::Added,
                mode_of_entry(*entry_mode),
                None,
                None,
            )
        }
        Change::Deletion {
            location,
            entry_mode,
            ..
        } => {
            if entry_mode.is_tree() {
                return None;
            }
            (
                location,
                None,
                FileStatus::Deleted,
                mode_of_entry(*entry_mode),
                None,
                None,
            )
        }
        Change::Modification {
            location,
            entry_mode,
            previous_entry_mode,
            ..
        } => {
            if entry_mode.is_tree() {
                return None;
            }
            let changed = (entry_mode.kind() != previous_entry_mode.kind())
                .then(|| mode_of_entry(*entry_mode));
            (
                location,
                None,
                FileStatus::Modified,
                mode_of_entry(*entry_mode),
                changed,
                None,
            )
        }
        Change::Rewrite {
            location,
            source_location,
            source_entry_mode,
            entry_mode,
            copy,
            diff,
            ..
        } => {
            // The figure gix accepted the pair by, as git's `R<nnn>`: a count of our own
            // could differ from git's and even sit under the threshold that let it through.
            let percent = diff.map_or(100, |stats| {
                // In 0.0..=1.0, so the cast cannot truncate or wrap.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let percent = (stats.similarity.clamp(0.0, 1.0) * 100.0).floor() as u32;
                percent
            });
            let changed =
                (entry_mode.kind() != source_entry_mode.kind()).then(|| mode_of_entry(*entry_mode));
            (
                location,
                Some(source_location.to_string()),
                if *copy {
                    FileStatus::Copied
                } else {
                    FileStatus::Renamed
                },
                mode_of_entry(*entry_mode),
                changed,
                Some(percent),
            )
        }
    };

    Some(FileEntry {
        path: path.to_string(),
        old_path,
        status,
        mode,
        mode_change,
        similarity,
        submodule: None,
    })
}

#[cfg(test)]
mod message_tests {
    use super::*;

    #[test]
    fn trailers_are_the_last_paragraph_of_key_value_lines() {
        let body = "Why.\n\nCo-authored-by: Ann <a@x>\nSigned-off-by: Bo\n  continued";
        assert_eq!(
            parse_trailers(body),
            [
                Trailer {
                    key: "Co-authored-by".into(),
                    value: "Ann <a@x>".into()
                },
                Trailer {
                    key: "Signed-off-by".into(),
                    value: "Bo continued".into()
                },
            ]
        );
        assert!(parse_trailers("Just prose: here.\nand more").is_empty());
    }

    #[test]
    fn a_latin1_message_is_decoded_by_its_header() {
        let (text, encoding) = decode_message(b"caf\xe9", Some("ISO-8859-1"), None);
        assert_eq!(text, "caf\u{e9}");
        assert_eq!(encoding.as_deref(), Some("windows-1252"));
        assert_eq!(decode_message(b"plain", None, None), ("plain".into(), None));
    }

    #[test]
    fn invalid_utf8_without_a_header_falls_back_to_windows_1252() {
        let (text, encoding) = decode_message(b"\xe9t\xe9", None, None);
        assert_eq!(text, "\u{e9}t\u{e9}");
        assert!(encoding.is_some());
    }
}
