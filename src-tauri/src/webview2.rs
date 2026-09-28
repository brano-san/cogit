//! What the installed WebView2 runtime is, which keys the window takes back from it, and which
//! browser behaviors it is stripped of.
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
    ICoreWebView2Settings3, ICoreWebView2Settings4, ICoreWebView2Settings5, ICoreWebView2Settings6,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_MENU, VK_RMENU, VK_SHIFT,
};
use windows_core::Interface as _;
use windows_core::{PCWSTR, PWSTR};

use crate::accelerators::Modifiers;

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

/// Turns the webview from a browser into an application surface, once per window (main and
/// every child). Host level, so it holds before any page script runs and cannot be undone by
/// a page that fails to load: page-level `preventDefault` stays only as a second line.
///
/// Off: the browser accelerator keys (`Ctrl+F/P/S/U/J/H/R/N/T/W/O/D/G/L`, `F3/F5/F7/F12`,
/// `Alt+Left/Right`, `Ctrl+Shift+I/R/…`: find, print, save, view-source, downloads, history,
/// reload, new window, devtools, caret browsing, back/forward), the default context menu, page
/// zoom (`Ctrl +/-/0`, `Ctrl+wheel`, pinch), swipe navigation, the status bar with link URLs,
/// and password and form autofill. Developer tools stay available in debug builds only.
///
/// The accelerator switch does not stop `keydown` reaching the page, nor
/// `AcceleratorKeyPressed`: the menu claims (`install_accelerators`) and the page's own key
/// handlers keep working; only the browser's default action for the key is gone.
///
/// A setting an old runtime lacks is logged and skipped; the others still apply.
pub fn harden<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    let installed = window.with_webview(|platform| {
        let core = match unsafe { platform.controller().CoreWebView2() } {
            Ok(core) => core,
            Err(err) => {
                tracing::warn!(error = %err, "no CoreWebView2 to strip of browser behavior");
                return;
            }
        };
        let settings = match unsafe { core.Settings() } {
            Ok(settings) => settings,
            Err(err) => {
                tracing::warn!(error = %err, "cannot read the WebView2 settings");
                return;
            }
        };
        let note = |name: &str, result: windows_core::Result<()>| {
            if let Err(err) = result {
                tracing::warn!(error = %err, setting = name, "cannot apply a WebView2 setting");
            }
        };

        unsafe {
            note(
                "AreDefaultContextMenusEnabled",
                settings.SetAreDefaultContextMenusEnabled(false),
            );
            note(
                "IsZoomControlEnabled",
                settings.SetIsZoomControlEnabled(false),
            );
            note("IsStatusBarEnabled", settings.SetIsStatusBarEnabled(false));
            note(
                "AreDevToolsEnabled",
                settings.SetAreDevToolsEnabled(cfg!(debug_assertions)),
            );
        }
        // Newer interfaces: each cast fails on a runtime that predates it.
        if let Ok(s) = settings.cast::<ICoreWebView2Settings3>() {
            note("AreBrowserAcceleratorKeysEnabled", unsafe {
                s.SetAreBrowserAcceleratorKeysEnabled(false)
            });
        }
        if let Ok(s) = settings.cast::<ICoreWebView2Settings4>() {
            unsafe {
                note(
                    "IsPasswordAutosaveEnabled",
                    s.SetIsPasswordAutosaveEnabled(false),
                );
                note(
                    "IsGeneralAutofillEnabled",
                    s.SetIsGeneralAutofillEnabled(false),
                );
            }
        }
        if let Ok(s) = settings.cast::<ICoreWebView2Settings5>() {
            note("IsPinchZoomEnabled", unsafe {
                s.SetIsPinchZoomEnabled(false)
            });
        }
        if let Ok(s) = settings.cast::<ICoreWebView2Settings6>() {
            note("IsSwipeNavigationEnabled", unsafe {
                s.SetIsSwipeNavigationEnabled(false)
            });
        }
    });

    if let Err(err) = installed {
        tracing::warn!(error = %err, "cannot reach the platform webview to strip browser behavior");
    }
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
    install(
        window,
        {
            let app = app.clone();
            move |pressed| {
                // Rebuilt per press rather than cached: it is a few dozen short strings, it
                // only runs for accelerator keys, and a cache would have to be invalidated
                // every time the user edits the keymap.
                app.state::<crate::key_capture::KeyCapture>().claim(|| {
                    let overrides = app.state::<crate::menu::Keymap>().current();
                    crate::accelerators::table(crate::menu::default_keymap_pairs(), &overrides)
                        .get(&pressed)
                        .cloned()
                })
            }
        },
        move |id| crate::dispatch_menu_command(&app, id),
    );
}

/// The same for a child window with a menu of its own (Investigate, Blame): its table is
/// fixed, `child_window::accelerator_table`, and a claimed key runs its menu item.
pub fn install_child_accelerators<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    table: std::collections::HashMap<crate::accelerators::Chord, String>,
) {
    if table.is_empty() {
        return;
    }
    let app = window.app_handle().clone();
    install(
        window,
        move |pressed| table.get(&pressed).cloned(),
        move |id| {
            crate::child_window::on_menu(&app, id);
        },
    );
}

fn install<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    claim: impl Fn(crate::accelerators::Chord) -> Option<String> + Send + 'static,
    run: impl Fn(&str) + Send + 'static,
) {
    let installed = window.with_webview(move |platform| {
        let controller = platform.controller();

        let mut token = 0_i64;
        let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_sender, args| {
            let Some(args) = args else {
                return Ok(());
            };

            if let Some(id) = pressed(&args).and_then(&claim) {
                // Told first, so the page never sees a key the window has taken.
                unsafe { args.SetHandled(true) }?;
                run(&id);
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

/// The chord this key press stands for, or `None` for one no menu can claim.
fn pressed(
    args: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2AcceleratorKeyPressedEventArgs,
) -> Option<crate::accelerators::Chord> {
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

    crate::accelerators::chord_of(
        Modifiers {
            ctrl: is_down(VK_CONTROL),
            shift: is_down(VK_SHIFT),
            alt: is_down(VK_MENU),
            right_alt: is_down(VK_RMENU),
        },
        u16::try_from(key).ok()?,
    )
}

/// The event says which key, never which modifiers, so they are read from the keyboard.
fn is_down(key: VIRTUAL_KEY) -> bool {
    let state = unsafe { GetKeyState(i32::from(key.0)) };
    (state as u16 & 0x8000) != 0
}
