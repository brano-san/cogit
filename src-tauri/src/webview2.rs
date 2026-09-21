//! What the installed WebView2 runtime is.
//!
//! The runtime updates itself on the user's machine without asking anyone, so "which
//! version was it" is the first question about any report that starts with "the interface
//! went blank". It belongs in the first line of the log, next to our own version.
//!
//! See doc/12-risks.md (R-88) for why this file is allowed `unsafe`.

// COM again: the loader has no safe entry point.
#![allow(unsafe_code)]

use webview2_com::Microsoft::Web::WebView2::Win32::GetAvailableCoreWebView2BrowserVersionString;
use windows_core::{PCWSTR, PWSTR};

/// `None` means the runtime is missing or too old to answer — which is itself the answer
/// to a report about a window that never painted.
#[must_use]
pub fn browser_version() -> Option<String> {
    let mut version = PWSTR::null();

    // A null folder asks about the installed runtime rather than a bundled copy.
    unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut version) }.ok()?;

    if version.is_null() {
        return None;
    }

    let text = webview2_com::take_pwstr(version);
    (!text.is_empty()).then_some(text)
}
