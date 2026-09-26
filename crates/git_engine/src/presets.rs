use crate::{GitError, Result};
use serde::Deserialize;

/// Data, not code: a new preset is a new TOML file. Embedded so a damaged install works.
const BUILTIN: &[&str] = &[
    include_str!("../presets/rust-fmt.toml"),
    include_str!("../presets/commit-message.toml"),
    include_str!("../presets/large-files.toml"),
    include_str!("../presets/secrets.toml"),
];

#[derive(Debug, Clone, Deserialize)]
pub struct PresetTool {
    pub command: String,
    pub install_hint: String,
    /// Searched before `PATH`: a GUI's environment often predates the tool (T10.4 rule 7).
    pub search_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub hook: String,
    pub description: String,
    #[serde(default)]
    pub config_files: Vec<String>,
    #[serde(default)]
    pub slow: bool,
    pub script: String,
    #[serde(default)]
    pub tool: Option<PresetTool>,
}

/// A user's own preset as the file `parse_preset` reads back. Written by the TOML library,
/// not by hand: any name and any script round-trip, emoji and `'''` in a docstring included.
pub fn preset_toml(
    id: &str,
    name: &str,
    hook: &str,
    description: &str,
    script: &str,
) -> Result<String> {
    #[derive(serde::Serialize)]
    struct Written<'a> {
        id: &'a str,
        name: &'a str,
        hook: &'a str,
        description: &'a str,
        script: &'a str,
    }
    toml::to_string(&Written {
        id,
        name,
        hook,
        description,
        script,
    })
    .map_err(|err| GitError::Internal(format!("cannot write the preset: {err}")))
}

pub fn parse_preset(text: &str) -> Result<Preset> {
    toml::from_str(text).map_err(|err| GitError::InvalidState(format!("bad preset: {err}")))
}

/// A preset that does not parse is skipped rather than hiding the whole catalogue.
#[must_use]
pub fn builtin_presets() -> Vec<Preset> {
    BUILTIN
        .iter()
        .filter_map(|text| match parse_preset(text) {
            Ok(preset) => Some(preset),
            Err(err) => {
                tracing::error!(error = ?err, context = "a built-in preset does not parse");
                None
            }
        })
        .collect()
}

/// The declared directories first, then `PATH` (doc/modules/M10-hooks.md, T10.4 rule 7).
#[must_use]
pub fn find_tool(tool: &PresetTool) -> Option<std::path::PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        ["exe", "cmd", "bat", ""]
            .iter()
            .map(|ext| {
                if ext.is_empty() {
                    tool.command.clone()
                } else {
                    format!("{}.{ext}", tool.command)
                }
            })
            .collect()
    } else {
        vec![tool.command.clone()]
    };

    for directory in &tool.search_paths {
        let Some(expanded) = expand(directory) else {
            continue;
        };
        for name in &names {
            let candidate = std::path::Path::new(&expanded).join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|directory| {
        names
            .iter()
            .map(|name| directory.join(name))
            .find(|candidate| candidate.is_file())
    })
}

/// Where `find_tool` looks, in its order: the declared directories as expanded, then
/// `PATH`, named as such.
#[must_use]
pub fn tool_search_places(tool: &PresetTool) -> Vec<String> {
    tool.search_paths
        .iter()
        .filter_map(|directory| expand(directory))
        .chain(["PATH".to_owned()])
        .collect()
}

/// An unset variable makes the whole entry meaningless, so the entry is skipped.
fn expand(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        let end = tail
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .unwrap_or(tail.len());
        let name = &tail[..end];
        if name.is_empty() {
            out.push('$');
        } else {
            out.push_str(&std::env::var(name).ok()?);
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    Some(out)
}

impl crate::RepoHandle {
    /// Writes the preset's script as its hook, replacing whatever was there.
    pub fn install_preset(&self, preset: &Preset) -> Result<()> {
        self.write_hook(&preset.hook, &preset.script)
    }
}
