//! Proxy and TLS keys of the user's global Git config, for Preferences ▸ Network (F-942).
//! The repository's own values go through `repo_settings`.

use crate::{CommandSink, GitError, GitOutput, Result};
use std::process::Output;

/// What Preferences ▸ Network offers, in order.
pub const NETWORK_CONFIG_KEYS: &[&str] = &["http.proxy", "http.sslVerify", "http.sslCAInfo"];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSetting {
    pub key: String,
    /// `None`: not set in the user's global config.
    pub value: Option<String>,
}

fn offered(key: &str) -> Option<&'static str> {
    NETWORK_CONFIG_KEYS
        .iter()
        .find(|known| known.eq_ignore_ascii_case(key))
        .copied()
}

/// Outside any repository: only `--global` is asked, and a broken one nearby cannot fail it.
fn run_global(args: &[&str]) -> Result<(GitOutput, Output)> {
    let command = crate::redact_command(args);
    let started = std::time::Instant::now();
    tracing::info!(%command, "running git");
    let place = std::env::temp_dir();
    let mut process = crate::runner::git_command();
    process.current_dir(&place).args(args);
    let output = crate::children::output(&mut process).map_err(crate::runner::not_started)?;
    let record = GitOutput::from_process(&place, command, &output, started);
    Ok((record, output))
}

/// `-z --get-regexp`: key and value split by a newline, a key alone meaning `true`. The
/// last line for a key is the one git applies.
fn parse(listing: &str) -> Vec<GlobalSetting> {
    let mut values: Vec<Option<String>> = vec![None; NETWORK_CONFIG_KEYS.len()];
    for entry in listing.split('\0').filter(|entry| !entry.is_empty()) {
        let (key, value) = entry.split_once('\n').unwrap_or((entry, "true"));
        if let Some(at) = NETWORK_CONFIG_KEYS
            .iter()
            .position(|known| known.eq_ignore_ascii_case(key))
        {
            values[at] = Some(value.to_owned());
        }
    }
    NETWORK_CONFIG_KEYS
        .iter()
        .zip(values)
        .map(|(key, value)| GlobalSetting {
            key: (*key).to_owned(),
            value,
        })
        .collect()
}

/// Every offered key, `None` where the global config does not set it.
pub fn global_network_settings() -> Result<Vec<GlobalSetting>> {
    let pattern = format!(
        "^({})$",
        NETWORK_CONFIG_KEYS
            .iter()
            .map(|key| key.to_ascii_lowercase().replace('.', "\\."))
            .collect::<Vec<_>>()
            .join("|")
    );
    let (record, output) = run_global(&["config", "--global", "-z", "--get-regexp", &pattern])?;
    match output.status.code() {
        Some(0) => Ok(parse(&String::from_utf8_lossy(&output.stdout))),
        // Nothing matched, or there is no global config file yet.
        Some(1) => Ok(parse("")),
        _ => Err(record.into()),
    }
}

/// Written with `git config --global`; empty or `None` removes the key. The journal shows a
/// proxy's password as `***` (`redact_command`).
pub fn set_global_setting(
    key: &str,
    value: Option<&str>,
    journal: Option<&CommandSink>,
) -> Result<()> {
    let Some(key) = offered(key) else {
        return Err(GitError::InvalidState(format!(
            "{key} is not a network setting"
        )));
    };
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    let args = match value {
        Some(value) => vec!["config", "--global", "--replace-all", "--", key, value],
        // Asked first so that nothing already absent shows up as a failed command.
        None if global_network_settings()?
            .iter()
            .any(|entry| entry.key == key && entry.value.is_some()) =>
        {
            vec!["config", "--global", "--unset-all", "--", key]
        }
        None => return Ok(()),
    };
    let (record, output) = run_global(&args)?;
    if let Some(sink) = journal {
        sink(record.clone());
    }
    if output.status.success() {
        Ok(())
    } else {
        Err(record.into())
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn the_last_value_of_a_key_wins_and_a_bare_key_is_true() {
        let found = parse("http.proxy\nhttp://a:1\0http.sslverify\0http.proxy\nhttp://b:2\0");
        assert_eq!(found[0].value.as_deref(), Some("http://b:2"));
        assert_eq!(found[1].value.as_deref(), Some("true"));
        assert_eq!(found[2].value, None);
        assert_eq!(found[2].key, "http.sslCAInfo");
    }
}
