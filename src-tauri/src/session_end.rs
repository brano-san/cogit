//! Holds the end of a Windows session while operations run, and tells the page (R-168).
#![allow(unsafe_code)]

use tauri::Manager as _;
use tauri_specta::Event as _;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Shutdown::{ShutdownBlockReasonCreate, ShutdownBlockReasonDestroy};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{WM_NCDESTROY, WM_QUERYENDSESSION};
use windows_core::HSTRING;

const SUBCLASS_ID: usize = 0x00c0_917e;

struct Context {
    app: tauri::AppHandle,
}

pub fn install(window: &tauri::WebviewWindow) {
    let hwnd = match window.hwnd() {
        Ok(hwnd) => hwnd,
        Err(err) => {
            tracing::warn!(error = %err, "no window handle; a shutdown will not wait for operations");
            return;
        }
    };
    let context = Box::into_raw(Box::new(Context {
        app: window.app_handle().clone(),
    }));
    let installed =
        unsafe { SetWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID, context as usize) };
    if installed.as_bool() {
        tracing::info!("session end watched");
    } else {
        drop(unsafe { Box::from_raw(context) });
        tracing::warn!("cannot watch the session end; a shutdown will not wait for operations");
    }
}

/// Drops the reason once the queue is empty, so the next shutdown goes straight through.
pub fn release(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let raw = hwnd.0 as isize;
    let _ = app.run_on_main_thread(move || {
        let _ = unsafe { ShutdownBlockReasonDestroy(HWND(raw as *mut _)) };
    });
}

unsafe extern "system" fn subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    match message {
        WM_QUERYENDSESSION => {
            let context = unsafe { &*(data as *const Context) };
            if let Some(reason) = blocker(&context.app) {
                hold(hwnd, &reason, &context.app);
                return LRESULT(0);
            }
            let _ = unsafe { ShutdownBlockReasonDestroy(hwnd) };
        }
        WM_NCDESTROY => unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID);
            drop(Box::from_raw(data as *mut Context));
        },
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn blocker(app: &tauri::AppHandle) -> Option<String> {
    app.try_state::<crate::AppContext>()?
        .state
        .session_end_blocker()
}

fn hold(hwnd: HWND, reason: &str, app: &tauri::AppHandle) {
    if let Err(err) = unsafe { ShutdownBlockReasonCreate(hwnd, &HSTRING::from(reason)) } {
        tracing::warn!(error = %err, "cannot name the reason the session end is held");
    }
    tracing::info!(%reason, "held the end of the session");
    let app = app.clone();
    let reason = reason.to_owned();
    tauri::async_runtime::spawn(async move {
        let _ = crate::SessionEnding { reason }.emit(&app);
    });
}
