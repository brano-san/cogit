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

const USER_THEME_FILE: &str = "user-theme.json";

/// `user-theme.json` beside the settings, as JSON text: a JSON object, or `{}` when the
/// file is absent, unreadable or not an object. It is the user's own, written by hand or
/// by a future settings screen, so a damaged one is warned about and ignored, never fatal.
#[must_use]
pub fn read_user_theme(config_dir: &Path) -> String {
    let path = config_dir.join(USER_THEME_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return "{}".into(),
        Err(err) => {
            tracing::warn!(?path, error = ?err, "user-theme.json cannot be read; ignored");
            return "{}".into();
        }
    };
    match decode(&bytes).map(|json| serde_json::from_str::<Value>(&json)) {
        Some(Ok(value @ Value::Object(_))) => value.to_string(),
        Some(Ok(_) | Err(_)) | None => {
            tracing::warn!(?path, "user-theme.json is not a JSON object; ignored");
            "{}".into()
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
    /// The bytes as they were, for the copy kept aside before it is replaced.
    Damaged(Vec<u8>),
}

/// The text of a file a person may have saved from any editor: UTF-8 with or without a
/// byte-order mark, or the UTF-16 that Windows PowerShell 5.1 writes. `None` is no text.
fn decode(bytes: &[u8]) -> Option<String> {
    let utf16 = |pairs: &[u8], unit: fn([u8; 2]) -> u16| {
        let (pairs, odd) = pairs.as_chunks::<2>();
        let units: Vec<u16> = pairs.iter().map(|pair| unit(*pair)).collect();
        if odd.is_empty() {
            String::from_utf16(&units).ok()
        } else {
            None
        }
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, u16::from_be_bytes),
        _ => {
            let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
            String::from_utf8(bytes.to_vec()).ok()
        }
    }
}

fn stored(config_dir: &Path) -> std::io::Result<Stored> {
    let bytes = match std::fs::read(file_in(config_dir)) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Stored::Missing),
        Err(err) => return Err(err),
    };
    Ok(
        match decode(&bytes).map(|json| serde_json::from_str(&json)) {
            Some(Ok(Value::Object(map))) => Stored::Document(map),
            _ => Stored::Damaged(bytes),
        },
    )
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
