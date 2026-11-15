//! `--add-to-path` for the per-user NSIS install: NSIS strings stop at 1024 characters, so the
//! PATH is edited here, whole, as the registry holds it (REG_EXPAND_SZ kept unexpanded).

fn same_entry(entry: &str, dir: &str) -> bool {
    let clean = |text: &str| {
        text.trim()
            .trim_matches('"')
            .trim_end_matches(['\\', '/'])
            .to_lowercase()
    };
    clean(entry) == clean(dir)
}

pub fn with_entry(value: &str, dir: &str) -> Option<String> {
    if value.split(';').any(|entry| same_entry(entry, dir)) {
        return None;
    }
    Some(if value.is_empty() || value.ends_with(';') {
        format!("{value}{dir}")
    } else {
        format!("{value};{dir}")
    })
}

pub fn without_entry(value: &str, dir: &str) -> Option<String> {
    let kept: Vec<&str> = value
        .split(';')
        .filter(|entry| !same_entry(entry, dir))
        .collect();
    (kept.len() != value.split(';').count()).then(|| kept.join(";"))
}

/// `Ok(false)`: nothing to change. The installer broadcasts `WM_SETTINGCHANGE`.
#[cfg(windows)]
pub fn change_user_path(dir: &str, add: bool) -> std::io::Result<bool> {
    use windows_registry::{CURRENT_USER, Type};
    let key = CURRENT_USER
        .create("Environment")
        .map_err(std::io::Error::other)?;
    let (value, ty) = match key.get_value("Path") {
        Ok(value) => {
            let ty = value.ty();
            (String::try_from(value).map_err(std::io::Error::other)?, ty)
        }
        // Only ERROR_FILE_NOT_FOUND is an empty PATH; else the write would replace the PATH.
        Err(err) if err.code().0 == 0x8007_0002_u32.cast_signed() => {
            if !add {
                return Ok(false);
            }
            (String::new(), Type::ExpandString)
        }
        Err(err) => return Err(std::io::Error::other(err)),
    };
    let changed = if add {
        with_entry(&value, dir)
    } else {
        without_entry(&value, dir)
    };
    let Some(changed) = changed else {
        return Ok(false);
    };
    let written = if ty == Type::String {
        key.set_string("Path", &changed)
    } else {
        key.set_expand_string("Path", &changed)
    };
    written.map(|()| true).map_err(std::io::Error::other)
}

#[cfg(test)]
#[path = "path_env_tests.rs"]
mod tests;
