//! A start-up failure that ends the process. A release build on Windows has no console, so the
//! old `writeln!(stderr)` went nowhere and the user saw nothing (SP-2): the reason is shown in a
//! message box, the one place a native dialog is acceptable before any window exists.

/// The error and its causes, once each: a `thiserror` display often repeats its source.
#[must_use]
pub fn message(err: &(dyn std::error::Error + 'static)) -> String {
    let mut text = err.to_string();
    let mut cause = err.source();
    while let Some(next) = cause {
        let next_text = next.to_string();
        if !text.contains(&next_text) {
            text = format!("{text}: {next_text}");
        }
        cause = next.source();
    }
    text
}

/// To stderr (a terminal run) and, on Windows, to a message box. Does not exit.
pub fn report(err: &(dyn std::error::Error + 'static)) {
    use std::io::Write as _;
    let text = message(err);
    let _ = writeln!(std::io::stderr(), "[cogit] fatal: {text}");
    #[cfg(windows)]
    native::show(&text);
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod native {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    use windows_core::HSTRING;

    pub fn show(text: &str) {
        unsafe {
            MessageBoxW(
                None,
                &HSTRING::from(text),
                &HSTRING::from("Cogit cannot start"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::message;

    #[test]
    fn the_message_carries_the_path_and_the_os_error_once() {
        let err = anyhow::Error::from(portable::Error::NotWritable {
            path: "X:\\Cogit-data\\config".into(),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Access is denied"),
        })
        .context("cannot start");
        let text = message(err.as_ref());
        assert!(text.contains("X:\\Cogit-data\\config"), "{text}");
        assert_eq!(text.matches("Access is denied").count(), 1, "{text}");
    }
}
