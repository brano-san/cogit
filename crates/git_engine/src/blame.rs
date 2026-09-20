use crate::{GitError, RepoHandle, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub line: u32,
    pub text: String,
    pub oid: String,
    pub summary: String,
    pub author: String,
    pub email: String,
    #[specta(type = specta_typescript::Number)]
    pub timestamp: i64,
}

impl RepoHandle {
    /// One entry per line, in file order.
    pub fn blame(&self, path: &str, rev: &str) -> Result<Vec<BlameLine>> {
        let suspect = self
            .repo
            .rev_parse_single(rev)
            .map_err(|err| GitError::InvalidState(format!("cannot resolve {rev}: {err}")))?
            .detach();

        let outcome = self
            .repo
            .blame_file(
                path.into(),
                suspect,
                gix::repository::blame_file::Options::default(),
            )
            .map_err(|err| GitError::InvalidState(format!("cannot blame {path}: {err}")))?;

        let text = String::from_utf8_lossy(&outcome.blob);
        let lines: Vec<&str> = text.lines().collect();
        let mut out: Vec<BlameLine> = Vec::with_capacity(lines.len());

        // Empty when there is no `.mailmap`, and empty again when the file is unparsable:
        // a mistyped mapping must not cost the user their blame.
        let mailmap = self.repo.open_mailmap();

        for entry in &outcome.entries {
            let oid = entry.commit_id.to_string();
            let details = self.commit_details(&oid)?;
            let (author, email) = canonical(&mailmap, &details.author);
            for offset in 0..entry.len.get() {
                let index = (entry.start_in_blamed_file + offset) as usize;
                out.push(BlameLine {
                    line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                    text: lines.get(index).copied().unwrap_or_default().to_owned(),
                    oid: oid.clone(),
                    summary: details.summary.clone(),
                    author: author.clone(),
                    email: email.clone(),
                    timestamp: details.author.timestamp,
                });
            }
        }

        out.sort_by_key(|line| line.line);
        Ok(out)
    }
}

/// The name and address `.mailmap` wants shown, or the ones the commit carries when the
/// file says nothing about them.
fn canonical(mailmap: &gix::mailmap::Snapshot, author: &crate::Signature) -> (String, String) {
    let resolved = mailmap.resolve(gix::actor::SignatureRef {
        name: gix::bstr::BStr::new(author.name.as_bytes()),
        email: gix::bstr::BStr::new(author.email.as_bytes()),
        time: "",
    });
    (resolved.name.to_string(), resolved.email.to_string())
}
