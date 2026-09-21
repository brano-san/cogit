use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

const FILE: &str = "settings.json";

/// Writes are read-modify-write, so two of them at once would otherwise drop a key.
static WRITING: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

fn file_in(config_dir: &Path) -> PathBuf {
    config_dir.join(FILE)
}

/// The whole settings document. Anything unreadable reads as empty: a damaged file must
/// not keep the application from starting with defaults.
#[must_use]
pub fn read_document(config_dir: &Path) -> Value {
    let Ok(text) = std::fs::read_to_string(file_in(config_dir)) else {
        return Value::Object(Map::new());
    };
    match serde_json::from_str(&text) {
        Ok(Value::Object(map)) => Value::Object(map),
        _ => Value::Object(Map::new()),
    }
}

/// Replaces one top-level key and leaves the rest of the document as it was.
pub fn write_key(config_dir: &Path, key: &str, value: Value) -> std::io::Result<()> {
    let _writing = WRITING.lock();

    std::fs::create_dir_all(config_dir)?;
    let mut document = match read_document(config_dir) {
        Value::Object(map) => map,
        _ => Map::new(),
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
