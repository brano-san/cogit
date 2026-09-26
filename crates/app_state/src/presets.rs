//! The hook preset catalogue: built-ins plus whatever the user saved, each told against
//! one repository so the panel can warn before an install, not after (M10 T10.4).

use git_engine::{GitError, Preset};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetStatus {
    pub id: String,
    pub name: String,
    pub hook: String,
    pub description: String,
    pub slow: bool,
    pub config_files: Vec<String>,
    /// The declared config files this repository does not have. A preset installed
    /// without them runs a tool that will pick up someone else's defaults.
    pub missing_config: Vec<String>,
    pub tool: Option<String>,
    pub install_hint: Option<String>,
    /// Where the tool was found, or `None` when it is not installed.
    pub tool_path: Option<String>,
    /// Where it was looked for, in order, `PATH` last; empty without a tool.
    pub searched: Vec<String>,
    pub user: bool,
}

pub fn status_for(preset: Preset, root: &Path, user: bool) -> PresetStatus {
    let missing_config = preset
        .config_files
        .iter()
        .filter(|name| !root.join(name).exists())
        .cloned()
        .collect();

    PresetStatus {
        tool_path: preset
            .tool
            .as_ref()
            .and_then(git_engine::find_tool)
            .map(|path| path.to_string_lossy().replace(char::from(92), "/")),
        searched: preset
            .tool
            .as_ref()
            .map(git_engine::tool_search_places)
            .unwrap_or_default()
            .into_iter()
            .map(|place| place.replace(char::from(92), "/"))
            .collect(),
        tool: preset.tool.as_ref().map(|tool| tool.command.clone()),
        install_hint: preset.tool.as_ref().map(|tool| tool.install_hint.clone()),
        id: preset.id,
        name: preset.name,
        hook: preset.hook,
        description: preset.description,
        slow: preset.slow,
        config_files: preset.config_files,
        missing_config,
        user,
    }
}

/// One unreadable file is skipped rather than hiding everything the user saved.
pub fn user_presets(dir: &Path) -> Vec<Preset> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<Preset> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "toml"))
        .filter_map(|entry| match std::fs::read_to_string(entry.path()) {
            Ok(text) => match git_engine::parse_preset(&text) {
                Ok(preset) => Some(preset),
                Err(error) => {
                    tracing::warn!(?error, path = ?entry.path(), "a saved preset does not parse");
                    None
                }
            },
            Err(_) => None,
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

pub fn write_preset(
    dir: &Path,
    id: &str,
    name: &str,
    hook: &str,
    description: &str,
    script: &str,
) -> Result<PathBuf, GitError> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{id}.toml"));
    let body = git_engine::preset_toml(id, name, hook, description, script)?;
    std::fs::write(&path, body)?;
    Ok(path)
}

/// Ids become file names, so anything that could climb out of the directory is refused.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
