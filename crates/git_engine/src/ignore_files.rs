use crate::{GitError, RepoHandle, Result};
use gix::bstr::{BStr, ByteSlice};
use std::path::{Path, PathBuf};

const MAX_HITS: usize = 500;
const GLOBAL_SOURCE: &str = "\u{0}global";
const EXCLUDE_SOURCE: &str = "\u{0}exclude";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum IgnoreScope {
    Folder,
    Exclude,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreFile {
    pub scope: IgnoreScope,
    pub dir: String,
    pub path: String,
    pub text: String,
    pub exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreDraft {
    pub scope: IgnoreScope,
    pub dir: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreHit {
    pub path: String,
    pub tracked: bool,
    pub scope: IgnoreScope,
    pub dir: String,
    pub line: u32,
    pub pattern: String,
}

#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct IgnorePreview {
    pub hits: Vec<IgnoreHit>,
    pub total: u32,
}

fn folder_dir(dir: &str) -> &str {
    dir.trim_matches('/')
}

fn shown(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn read_text(path: &Path) -> Result<Option<String>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).replace("\r\n", "\n"))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

impl RepoHandle {
    fn folder_file(&self, dir: &str) -> PathBuf {
        let dir = folder_dir(dir);
        let base = if dir.is_empty() {
            self.root().to_path_buf()
        } else {
            self.root().join(dir)
        };
        base.join(".gitignore")
    }

    fn exclude_file(&self) -> PathBuf {
        self.repo.common_dir().join("info").join("exclude")
    }

    fn global_file(&self) -> Result<PathBuf> {
        let configured = self
            .repo
            .config_snapshot()
            .trusted_path("core.excludesFile")
            .map_err(|err| {
                GitError::InvalidState(format!("core.excludesFile cannot be read: {err}"))
            })?;
        configured
            .or_else(|| gix::path::env::xdg_config("ignore", &mut gix::path::env::var))
            .ok_or_else(|| {
                GitError::InvalidState("no home folder for the global ignore file".to_owned())
            })
    }

    fn ignore_file_path(&self, scope: IgnoreScope, dir: &str) -> Result<PathBuf> {
        match scope {
            IgnoreScope::Folder => Ok(self.folder_file(dir)),
            IgnoreScope::Exclude => Ok(self.exclude_file()),
            IgnoreScope::Global => self.global_file(),
        }
    }

    pub fn ignore_files(&self, dir: &str) -> Result<Vec<IgnoreFile>> {
        [
            IgnoreScope::Folder,
            IgnoreScope::Exclude,
            IgnoreScope::Global,
        ]
        .into_iter()
        .map(|scope| {
            let path = self.ignore_file_path(scope, dir)?;
            let text = read_text(&path)?;
            Ok(IgnoreFile {
                scope,
                dir: if scope == IgnoreScope::Folder {
                    folder_dir(dir).to_owned()
                } else {
                    String::new()
                },
                path: shown(&path),
                exists: text.is_some(),
                text: text.unwrap_or_default(),
            })
        })
        .collect()
    }

    pub fn save_ignore_file(&self, draft: &IgnoreDraft) -> Result<()> {
        let path = self.ignore_file_path(draft.scope, &draft.dir)?;
        let crlf = std::fs::read(&path).is_ok_and(|bytes| bytes.find(b"\r\n").is_some());
        let mut text = draft.text.replace("\r\n", "\n");
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        if crlf {
            text = text.replace('\n', "\r\n");
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, text)?;
        Ok(())
    }

    fn ignore_search(
        &self,
        drafts: &[IgnoreDraft],
        tracked_folders: Vec<String>,
    ) -> Result<gix::ignore::Search> {
        let parse = gix::ignore::search::Ignore::default();
        let drafted = |scope: IgnoreScope, dir: &str| {
            drafts
                .iter()
                .find(|draft| {
                    draft.scope == scope
                        && (scope != IgnoreScope::Folder
                            || folder_dir(&draft.dir) == folder_dir(dir))
                })
                .map(|draft| draft.text.clone())
        };
        let text_of = |scope: IgnoreScope, dir: &str| -> Result<String> {
            match drafted(scope, dir) {
                Some(text) => Ok(text),
                None => Ok(read_text(&self.ignore_file_path(scope, dir)?)?.unwrap_or_default()),
            }
        };

        let mut search = gix::ignore::Search::default();
        search.add_patterns_buffer(
            text_of(IgnoreScope::Global, "")?.as_bytes(),
            GLOBAL_SOURCE,
            None,
            parse,
        );
        search.add_patterns_buffer(
            text_of(IgnoreScope::Exclude, "")?.as_bytes(),
            EXCLUDE_SOURCE,
            None,
            parse,
        );

        let mut folders: Vec<String> = tracked_folders
            .into_iter()
            .chain(
                drafts
                    .iter()
                    .filter(|draft| draft.scope == IgnoreScope::Folder)
                    .map(|draft| folder_dir(&draft.dir).to_owned()),
            )
            .collect();
        folders.sort_by_key(|dir| {
            (
                dir.split('/').filter(|part| !part.is_empty()).count(),
                dir.clone(),
            )
        });
        folders.dedup();
        for dir in folders {
            let text = text_of(IgnoreScope::Folder, &dir)?;
            search.add_patterns_buffer(
                text.as_bytes(),
                self.folder_file(&dir),
                Some(self.root()),
                parse,
            );
        }
        Ok(search)
    }

    fn source_of(&self, source: Option<&Path>) -> (IgnoreScope, String) {
        match source {
            Some(path) if path == Path::new(GLOBAL_SOURCE) => (IgnoreScope::Global, String::new()),
            Some(path) if path == Path::new(EXCLUDE_SOURCE) => {
                (IgnoreScope::Exclude, String::new())
            }
            Some(path) => {
                let dir = path
                    .parent()
                    .and_then(|parent| parent.strip_prefix(self.root()).ok())
                    .map(shown)
                    .unwrap_or_default();
                (IgnoreScope::Folder, dir)
            }
            None => (IgnoreScope::Folder, String::new()),
        }
    }

    pub fn ignore_preview(&self, drafts: &[IgnoreDraft]) -> Result<IgnorePreview> {
        let index = self
            .repo
            .index_or_empty()
            .map_err(|err| GitError::Internal(format!("cannot read the index: {err}")))?;
        let tracked_folders = index
            .entries()
            .iter()
            .map(|entry| entry.path(&index))
            .filter(|path| *path == ".gitignore" || path.ends_with(b"/.gitignore"))
            .map(|path| {
                path[..path.len() - ".gitignore".len()]
                    .to_str_lossy()
                    .trim_end_matches('/')
                    .to_owned()
            })
            .collect();
        let search = self.ignore_search(drafts, tracked_folders)?;
        let case = if self.repo.config_snapshot().boolean("core.ignoreCase") == Some(true) {
            gix::glob::pattern::Case::Fold
        } else {
            gix::glob::pattern::Case::Sensitive
        };
        let untracked = self.read_git(&["ls-files", "-z", "--others", "--directory"])?;
        let candidates = untracked
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(|path| (path.as_bytes().as_bstr(), false))
            .chain(
                index
                    .entries()
                    .iter()
                    .map(|entry| (entry.path(&index), true)),
            );

        let mut preview = IgnorePreview::default();
        for (path, is_tracked) in candidates {
            let is_dir = path.ends_with(b"/");
            let relative: &BStr = if is_dir {
                path[..path.len() - 1].as_bstr()
            } else {
                path
            };
            let Some(found) = search.pattern_matching_relative_path(relative, Some(is_dir), case)
            else {
                continue;
            };
            if found.pattern.is_negative() {
                continue;
            }
            preview.total += 1;
            if preview.hits.len() < MAX_HITS {
                let (scope, dir) = self.source_of(found.source);
                preview.hits.push(IgnoreHit {
                    path: path.to_str_lossy().into_owned(),
                    tracked: is_tracked,
                    scope,
                    dir,
                    line: u32::try_from(found.sequence_number).unwrap_or(u32::MAX),
                    pattern: found.pattern.to_string(),
                });
            }
        }
        Ok(preview)
    }
}
