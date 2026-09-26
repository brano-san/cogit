use std::path::{Path, PathBuf};

const FILE_NAME: &str = "cogit-third-party-licenses.txt";

/// A dev build is served by the dev server, which bundles nothing: no frontend list.
#[must_use]
pub fn document(version: &str, crates: &str, frontend: Option<&str>) -> String {
    let frontend = frontend.map(str::trim).filter(|text| !text.is_empty());
    let mut out = format!(
        "Third-party licenses in Cogit {version}\n\
         Generated when this build was made, from Cargo metadata and the frontend bundle.\n\n"
    );
    out.push_str(crates.trim_end());
    out.push_str("\n\n");
    match frontend {
        Some(list) => out.push_str(list),
        None => out.push_str(
            "Frontend packages\n\nListed only in a release build: \
             this one was served by the development server.",
        ),
    }
    out.push('\n');
    out
}

pub fn write(dir: &Path, text: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(FILE_NAME);
    std::fs::write(&path, text)?;
    Ok(path)
}
