use ignore::gitignore::{Gitignore, GitignoreBuilder};
use rayon::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Directories that hold thousands of files and never the repository the user meant.
/// A backstop under `.gitignore`, for the folders nobody writes a rule about.
const SKIP: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    ".svn",
    ".hg",
    "__pycache__",
    ".venv",
    "venv",
    ".gradle",
    ".cargo",
    ".rustup",
    "Library",
    "AppData",
    "$RECYCLE.BIN",
    "System Volume Information",
];

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub path: PathBuf,
    pub name: String,
    pub bare: bool,
}

#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Levels below the scanned folder; the folder itself is depth 0.
    pub max_depth: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self { max_depth: 6 }
    }
}

/// Walks `root` in parallel and reports every repository it can reach. A repository is a
/// leaf: the walk does not enter one, so a checkout full of vendored clones stays cheap.
pub fn scan(root: &Path, options: &ScanOptions, on_found: impl FnMut(Found) + Send) {
    let sink = Mutex::new(on_found);
    walk(root, 0, options, &sink, &[]);
}

fn walk<F: FnMut(Found) + Send>(
    dir: &Path,
    depth: usize,
    options: &ScanOptions,
    sink: &Mutex<F>,
    ignores: &[Arc<Gitignore>],
) {
    if depth > options.max_depth {
        return;
    }
    if let Some(found) = repository_at(dir) {
        if let Ok(mut emit) = sink.lock() {
            emit(found);
        }
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    // The rules of this directory apply to everything below it, so the chain grows as the
    // walk descends and the innermost file has the final say, exactly as git reads them.
    let chain: Vec<Arc<Gitignore>> = match gitignore_at(dir) {
        Some(local) => ignores.iter().cloned().chain([local]).collect(),
        None => ignores.to_vec(),
    };

    let children: Vec<PathBuf> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            // `file_type` comes free with the directory listing; `metadata` would be a
            // second syscall per entry, and a symlink could send the walk round a loop.
            entry.file_type().ok()?.is_dir().then(|| entry.path())
        })
        .filter(|path| !is_skipped(path) && !is_ignored(path, &chain))
        .collect();

    children
        .par_iter()
        .for_each(|child| walk(child, depth + 1, options, sink, &chain));
}

/// A `.gitignore` that does not parse is no reason to abandon the scan.
fn gitignore_at(dir: &Path) -> Option<Arc<Gitignore>> {
    let file = dir.join(".gitignore");
    if !file.is_file() {
        return None;
    }
    let mut builder = GitignoreBuilder::new(dir);
    if let Some(error) = builder.add(&file) {
        tracing::debug!(?error, path = ?file, "a .gitignore was not usable for the scan");
        return None;
    }
    builder.build().ok().map(Arc::new)
}

/// Innermost first: the deepest file that says anything about this path decides.
fn is_ignored(path: &Path, chain: &[Arc<Gitignore>]) -> bool {
    for rules in chain.iter().rev() {
        match rules.matched(path, true) {
            ignore::Match::Ignore(_) => return true,
            ignore::Match::Whitelist(_) => return false,
            ignore::Match::None => (),
        }
    }
    false
}

fn is_skipped(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SKIP.iter().any(|skip| skip.eq_ignore_ascii_case(name)))
}

/// A worktree or submodule keeps a `.git` **file**, so the kind is not checked.
fn repository_at(dir: &Path) -> Option<Found> {
    let bare = !dir.join(".git").exists() && is_bare(dir);
    if !bare && !dir.join(".git").exists() {
        return None;
    }
    Some(Found {
        path: dir.to_path_buf(),
        name: display_name(dir),
        bare,
    })
}

fn is_bare(dir: &Path) -> bool {
    dir.join("HEAD").is_file() && dir.join("objects").is_dir() && dir.join("refs").is_dir()
}

/// A drive root has no file name; showing an empty row would be worse than showing the path.
fn display_name(dir: &Path) -> String {
    dir.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string())
}
