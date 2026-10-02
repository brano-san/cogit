//! The first thing `run()` does on Windows: is the WebView2 Runtime there? Without it Tauri
//! cannot create a window and the user would see nothing at all, so this is the one place
//! a native message box is acceptable: there is no webview to draw a dialog in (R-701).

// Pure and compiled everywhere, so the decision and the text are tested off Windows too.
#![cfg_attr(not(windows), allow(dead_code))]

pub const DOWNLOAD_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

pub const TITLE: &str = "Cogit needs the WebView2 Runtime";

/// What `GetAvailableCoreWebView2BrowserVersionString` answered: the loader reads the same
/// `EdgeUpdate\Clients` keys the installer writes, and `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER`
/// first. An error, an empty string and an uninstalled `0.0.0.0` all mean there is none.
#[must_use]
pub fn is_present(version: Option<&str>) -> bool {
    version.is_some_and(|version| {
        let version = version.trim();
        !version.is_empty() && version != "0.0.0.0"
    })
}

#[must_use]
pub fn message() -> String {
    format!(
        "Cogit draws its window with Microsoft Edge WebView2. It is preinstalled on Windows 11 \
         and most Windows 10 systems, but it was not found on this computer, so Cogit cannot \
         start.\n\nDownload the WebView2 Runtime from:\n{DOWNLOAD_URL}\n\n\
         Open the download page now?"
    )
}

/// Returns when the runtime is there; otherwise asks, optionally opens the download page,
/// and ends the process.
#[cfg(windows)]
pub fn require() {
    if is_present(crate::webview2::browser_version().as_deref()) {
        return;
    }
    native::offer_download();
    std::process::exit(1);
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod native {
    use super::{DOWNLOAD_URL, TITLE, message};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::{
        IDYES, MB_ICONERROR, MB_YESNO, MessageBoxW, SW_SHOWNORMAL,
    };
    use windows_core::{HSTRING, w};

    pub fn offer_download() {
        let answer = unsafe {
            MessageBoxW(
                None,
                &HSTRING::from(message()),
                &HSTRING::from(TITLE),
                MB_YESNO | MB_ICONERROR,
            )
        };
        if answer == IDYES {
            unsafe {
                ShellExecuteW(
                    None,
                    w!("open"),
                    &HSTRING::from(DOWNLOAD_URL),
                    None,
                    None,
                    SW_SHOWNORMAL,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_runtime_is_present_only_with_a_real_version() {
        assert!(is_present(Some("126.0.2592.87")));
        assert!(is_present(Some(" 99.0.1150.30 ")));
        assert!(!is_present(None));
        assert!(!is_present(Some("")));
        assert!(!is_present(Some("  ")));
        assert!(!is_present(Some("0.0.0.0")));
    }

    #[test]
    fn the_message_explains_and_names_the_download_page() {
        let text = message();
        assert!(text.contains("WebView2"));
        assert!(text.contains("Windows 11"));
        assert!(text.contains(DOWNLOAD_URL));
        assert!(text.ends_with("Open the download page now?"));
    }
}
