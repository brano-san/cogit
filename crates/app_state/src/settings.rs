use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

const FILE: &str = "settings.json";

/// Writes are read-modify-write, so two of them at once would otherwise drop a key.
static WRITING: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

fn file_in(config_dir: &Path) -> PathBuf {
    config_dir.join(FILE)
}

#[must_use]
pub fn path(config_dir: &Path) -> PathBuf {
    file_in(config_dir)
}

/// The whole settings document. Anything unreadable reads as empty: a damaged file must
/// not keep the application from starting with defaults.
#[must_use]
pub fn read_document(config_dir: &Path) -> Value {
    match stored(config_dir) {
        Ok(Stored::Document(map)) => Value::Object(map),
        Ok(Stored::Missing | Stored::Damaged(_)) => Value::Object(Map::new()),
        Err(err) => {
            tracing::error!(error = ?err, context = "reading settings.json; defaults for now");
            Value::Object(Map::new())
        }
    }
}

/// Preferences ▸ Git executable, read before the first git runs. Empty or plain `git`
/// is the one on PATH.
#[must_use]
pub fn read_git_program(config_dir: &Path) -> Option<PathBuf> {
    let document = read_document(config_dir);
    let program = document.get("settings")?.get("gitPath")?.as_str()?.trim();
    (!program.is_empty() && program != "git").then(|| PathBuf::from(program))
}

enum Stored {
    Document(Map<String, Value>),
    Missing,
    /// The text as it was, for the copy kept aside before it is replaced.
    Damaged(String),
}

fn stored(config_dir: &Path) -> std::io::Result<Stored> {
    let text = match std::fs::read_to_string(file_in(config_dir)) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Stored::Missing),
        Err(err) => return Err(err),
    };
    // An editor that saves with a byte-order mark leaves the JSON as it was.
    let json = text.strip_prefix('\u{feff}').unwrap_or(&text);
    Ok(match serde_json::from_str(json) {
        Ok(Value::Object(map)) => Stored::Document(map),
        _ => Stored::Damaged(text),
    })
}

/// Replaces one top-level key and leaves the rest of the document as it was.
pub fn write_key(config_dir: &Path, key: &str, value: Value) -> std::io::Result<()> {
    let _writing = WRITING.lock();

    std::fs::create_dir_all(config_dir)?;
    // A file that cannot be read right now (locked by a scanner, say) is not an empty one:
    // writing over it would lose every other setting.
    let mut document = match stored(config_dir)? {
        Stored::Document(map) => map,
        Stored::Missing => Map::new(),
        Stored::Damaged(text) => {
            let aside = config_dir.join(format!("{FILE}.damaged"));
            tracing::warn!(
                ?aside,
                "settings.json does not parse; kept aside and started afresh"
            );
            std::fs::write(&aside, text)?;
            Map::new()
        }
    };
    document.insert(key.to_owned(), value);

    let text = serde_json::to_string_pretty(&Value::Object(document))
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;

    // Through a neighbour and a rename: a crash mid-write leaves the old file whole
    // rather than a half-written one the next start cannot parse.
    let temporary = config_dir.join(format!("{FILE}.writing"));
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, file_in(config_dir))
}
