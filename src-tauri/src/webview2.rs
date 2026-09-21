//! What the installed WebView2 runtime is, and which keys the window takes back from it.
//!
//! The runtime updates itself on the user's machine without asking anyone, so "which
//! version was it" is the first question about any report that starts with "the interface
//! went blank". It belongs in the first line of the log, next to our own version.
//!
//! See doc/12-risks.md (R-94) for why this file is allowed `unsafe`.

// COM again: neither the loader nor the accelerator event has a safe entry point.
#![allow(unsafe_code)]

use tauri::Manager as _;
use webview2_com::AcceleratorKeyPressedEventHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_KEY_EVENT_KIND, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN,
    COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN, GetAvailableCoreWebView2BrowserVersionString,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows_core::{PCWSTR, PWSTR};

use crate::accelerators::Chord;

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

/// Gives the window its menu accelerators back (problem 3).
///
/// While the page has focus, every key goes into the webview and the window's own
/// accelerator table never runs — `Ctrl+1` reaches the document and the menu item stays
/// silent. `AcceleratorKeyPressed` fires before the page sees the key, which is the one
/// place where the window can still claim it.
///
/// **Only what the menu declares is claimed.** Everything else is left `Handled = false`
/// and reaches the page untouched, which is what keeps `Ctrl+A`, the arrows, `Enter` and
/// `Ctrl+F` working inside whichever panel has focus. `accelerators::table` is the whole
/// rule, and it is built from the same list the menu bar is built from, so the two cannot
/// drift apart.
pub fn install_accelerators(window: &tauri::WebviewWindow) {
    let app = window.app_handle().clone();

    let installed = window.with_webview(move |platform| {
        let controller = platform.controller();

        let mut token = 0_i64;
        let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_sender, args| {
            let Some(args) = args else {
                return Ok(());
            };

            if let Some(id) = claimed(&app, &args) {
                // Told first, so the page never sees a key the window has taken.
                unsafe { args.SetHandled(true) }?;
                crate::dispatch_menu_command(&app, &id);
            }

            Ok(())
        }));

        if let Err(err) = unsafe { controller.add_AcceleratorKeyPressed(&handler, &mut token) } {
            tracing::warn!(error = %err, "cannot watch accelerator keys; menu shortcuts will not fire");
        } else {
            tracing::info!("window accelerators installed");
        }
    });

    if let Err(err) = installed {
        tracing::warn!(error = %err, "cannot reach the platform webview for accelerators");
    }
}

/// The menu id this key press stands for, or `None` to leave it to the page.
fn claimed(
    app: &tauri::AppHandle,
    args: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2AcceleratorKeyPressedEventArgs,
) -> Option<String> {
    let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
    unsafe { args.KeyEventKind(&mut kind) }.ok()?;

    // Key-up would fire the command a second time, and the page still needs to see it.
    if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
        && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
    {
        return None;
    }

    let mut key = 0_u32;
    unsafe { args.VirtualKey(&mut key) }.ok()?;

    let pressed = Chord {
        ctrl: is_down(VK_CONTROL),
        shift: is_down(VK_SHIFT),
        alt: is_down(VK_MENU),
        key: u16::try_from(key).ok()?,
    };

    // Rebuilt per press rather than cached: it is a few dozen short strings, it only runs
    // for accelerator keys, and a cache would have to be invalidated every time the user
    // edits the keymap.
    let overrides = app.state::<crate::menu::Keymap>().current();
    let table = crate::accelerators::table(crate::menu::default_keymap_pairs(), &overrides);

    table.get(&pressed).cloned()
}

/// The event says which key, never which modifiers, so they are read from the keyboard.
fn is_down(key: VIRTUAL_KEY) -> bool {
    let state = unsafe { GetKeyState(i32::from(key.0)) };
    (state as u16 & 0x8000) != 0
}
