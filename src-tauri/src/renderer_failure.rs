//! Catching the renderer's death, so the user never meets Edge's own error page.
//!
//! The host process outlives its renderer. That is why a dead renderer leaves the native
//! menu working while the UI is replaced by a Chromium page, in the system language,
//! apologising about "this page" to an application that has no pages. WebView2 announces
//! the death first, through `ICoreWebView2::add_ProcessFailed`, and this is what listens.
//!
//! See doc/12-risks.md (R-94).

// Every WebView2 call below is COM, and the platform offers no safe door to this event.
// The workspace denies `unsafe_code` so that the Git paths cannot contain any; this file
// is the documented exception, and it holds no Git logic of its own.
#![allow(unsafe_code)]

use tauri::{Emitter as _, Manager as _};
use tauri_plugin_dialog::{DialogExt as _, MessageDialogButtons, MessageDialogKind};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED,
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED,
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE,
    COREWEBVIEW2_PROCESS_FAILED_REASON, COREWEBVIEW2_PROCESS_FAILED_REASON_CRASHED,
    COREWEBVIEW2_PROCESS_FAILED_REASON_LAUNCH_FAILED,
    COREWEBVIEW2_PROCESS_FAILED_REASON_OUT_OF_MEMORY,
    COREWEBVIEW2_PROCESS_FAILED_REASON_PROFILE_DELETED,
    COREWEBVIEW2_PROCESS_FAILED_REASON_TERMINATED, COREWEBVIEW2_PROCESS_FAILED_REASON_UNEXPECTED,
    COREWEBVIEW2_PROCESS_FAILED_REASON_UNRESPONSIVE, ICoreWebView2ProcessFailedEventArgs,
    ICoreWebView2ProcessFailedEventArgs2,
};
use webview2_com::ProcessFailedEventHandler;
use windows_core::Interface as _;

/// What died and why, reduced to what the log line and the dialog need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: &'static str,
    pub reason: &'static str,
    pub exit_code: i32,
    /// Which child it was: `"renderer"`, `"gpu"`, and so on. Empty for the render process,
    /// which is what WebView2 reports for the one that matters most.
    pub description: String,
}

/// Names for the log. The numeric values mean nothing to whoever reads `cogit.log`.
fn kind_name(kind: COREWEBVIEW2_PROCESS_FAILED_KIND) -> &'static str {
    match kind {
        COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED => "browser-process-exited",
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED => "render-process-exited",
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE => {
            "render-process-unresponsive"
        }
        _ => "other-process-exited",
    }
}

fn reason_name(reason: COREWEBVIEW2_PROCESS_FAILED_REASON) -> &'static str {
    match reason {
        COREWEBVIEW2_PROCESS_FAILED_REASON_OUT_OF_MEMORY => "out-of-memory",
        COREWEBVIEW2_PROCESS_FAILED_REASON_CRASHED => "crashed",
        COREWEBVIEW2_PROCESS_FAILED_REASON_LAUNCH_FAILED => "launch-failed",
        COREWEBVIEW2_PROCESS_FAILED_REASON_TERMINATED => "terminated",
        COREWEBVIEW2_PROCESS_FAILED_REASON_PROFILE_DELETED => "profile-deleted",
        COREWEBVIEW2_PROCESS_FAILED_REASON_UNRESPONSIVE => "unresponsive",
        COREWEBVIEW2_PROCESS_FAILED_REASON_UNEXPECTED => "unexpected",
        _ => "unknown",
    }
}

/// An unresponsive renderer is still alive, so it is offered time rather than a reload:
/// killing it would throw away whatever it is slowly working on.
fn is_hung(kind: &str) -> bool {
    kind == "render-process-unresponsive"
}

/// Only the render process can be brought back by reloading. If the browser process is
/// gone the whole webview is, and there is nothing left here to reload into; a GPU or
/// utility process WebView2 restarts itself, and the page never went away.
fn can_reload(kind: &str) -> bool {
    kind == "render-process-exited"
}

/// Reads the event arguments. `…Args2` carries the reason, the exit code and the process
/// description; it has shipped since WebView2 1.0.992, well below our floor, but a missing
/// interface must still cost a field rather than the whole report.
fn read(args: Option<&ICoreWebView2ProcessFailedEventArgs>) -> Failure {
    let Some(args) = args else {
        return Failure {
            kind: "unknown",
            reason: "unknown",
            exit_code: 0,
            description: String::new(),
        };
    };

    let mut raw_kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
    let kind = if unsafe { args.ProcessFailedKind(&mut raw_kind) }.is_ok() {
        kind_name(raw_kind)
    } else {
        "unknown"
    };

    let Ok(args2) = args.cast::<ICoreWebView2ProcessFailedEventArgs2>() else {
        return Failure {
            kind,
            reason: "unknown",
            exit_code: 0,
            description: String::new(),
        };
    };

    let mut raw_reason = COREWEBVIEW2_PROCESS_FAILED_REASON::default();
    let reason = if unsafe { args2.Reason(&mut raw_reason) }.is_ok() {
        reason_name(raw_reason)
    } else {
        "unknown"
    };

    let mut exit_code = 0_i32;
    let _ = unsafe { args2.ExitCode(&mut exit_code) };

    let mut described = windows_core::PWSTR::null();
    let description = if unsafe { args2.ProcessDescription(&mut described) }.is_ok() {
        webview2_com::take_pwstr(described)
    } else {
        String::new()
    };

    Failure {
        kind,
        reason,
        exit_code,
        description,
    }
}

/// Subscribes the window's webview. A failure to subscribe is logged and nothing more:
/// the app works without this, it just loses its manners when the renderer dies.
pub fn install(window: &tauri::WebviewWindow) {
    let app = window.app_handle().clone();
    let started = std::time::Instant::now();

    let subscribed = window.with_webview(move |platform| {
        let controller = platform.controller();
        let core = match unsafe { controller.CoreWebView2() } {
            Ok(core) => core,
            Err(err) => {
                tracing::warn!(error = %err, "no CoreWebView2 to watch for process failures");
                return;
            }
        };

        let mut token = 0_i64;
        let handler = ProcessFailedEventHandler::create(Box::new(move |_sender, args| {
            let failure = read(args.as_ref());
            announce(&app, &failure, started.elapsed());
            Ok(())
        }));

        if let Err(err) = unsafe { core.add_ProcessFailed(&handler, &mut token) } {
            tracing::warn!(error = %err, "cannot subscribe to WebView2 ProcessFailed");
        }
    });

    if let Err(err) = subscribed {
        tracing::warn!(error = %err, "cannot reach the platform webview");
    }
}

/// Writes the record, then asks the user what to do about it.
fn announce(app: &tauri::AppHandle, failure: &Failure, uptime: std::time::Duration) {
    tracing::error!(
        kind = failure.kind,
        reason = failure.reason,
        exit_code = failure.exit_code,
        process = failure.description.as_str(),
        uptime_s = uptime.as_secs(),
        "the webview lost a process"
    );

    // The renderer is gone, so nothing in the page can report this: the record has to be
    // written from here, and the figures from the host-side sampler are already in the
    // same log under `kind=procmem`.
    let _ = app.emit("renderer-failed", failure.kind);

    // Everything below has to leave this callback first. It runs inside the COM event
    // dispatch on the UI thread, and a dialog opened from there never reaches the screen.
    let handle = app.clone();
    let failure = failure.clone();

    let queued = app.run_on_main_thread(move || {
        if is_hung(failure.kind) {
            offer_wait(&handle);
            return;
        }
        if can_reload(failure.kind) {
            // The reload comes before the explanation, not after it. WebView2 puts its own
            // error page up the moment the renderer dies — a page in the system language,
            // about "this page", in an application that has none. Reloading first is what
            // makes sure the user never reads it.
            reload_webview(&handle);
            offer_reload(&handle, &failure);
        }
    });

    if let Err(err) = queued {
        tracing::error!(error = %err, "cannot answer a renderer failure on the main thread");
    }
}

/// Shown after the reload, never instead of it, so the wording is in the past tense: by
/// the time the user reads this the interface is already back.
fn offer_reload(app: &tauri::AppHandle, failure: &Failure) {
    let lead = if failure.reason == "out-of-memory" {
        "Не хватило памяти для отображения интерфейса."
    } else {
        "Отображение интерфейса аварийно завершилось."
    };

    let handle = app.clone();
    app.dialog()
        .message(format!(
            "{lead} Интерфейс перезагружен.\n\n\
             Ничего из вашей работы не потеряно.\n\
             Репозиторий и выбранный коммит восстановлены."
        ))
        .title("Cogit")
        .kind(MessageDialogKind::Error)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Продолжить".to_owned(),
            "Открыть лог".to_owned(),
        ))
        .show(move |carry_on| {
            if !carry_on {
                reveal_log(&handle);
            }
        });
}

fn offer_wait(app: &tauri::AppHandle) {
    let handle = app.clone();
    app.dialog()
        .message(
            "Интерфейс перестал отвечать. Возможно, идёт тяжёлая операция — \
             её стоит подождать.\n\nПерезагрузка прервёт то, чем он занят.",
        )
        .title("Cogit")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Подождать".to_owned(),
            "Перезагрузить интерфейс".to_owned(),
        ))
        .show(move |wait| {
            if !wait {
                reload_webview(&handle);
            }
        });
}

/// The session lives in the webview's own storage on disk, so a reload comes back to the
/// repository and the commit that were open. Nothing here has to carry that across.
fn reload_webview(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    // Not `eval`: the process that would have run the script is the one that just died.
    if let Err(err) = window.reload() {
        tracing::error!(error = %err, "cannot reload the webview after a process failure");
    }
}

fn reveal_log(app: &tauri::AppHandle) {
    let Some(context) = app.try_state::<crate::AppContext>() else {
        return;
    };
    let path = app_state::logging::latest_part(&context.log_path);
    if let Err(err) = tauri_plugin_opener::OpenerExt::opener(app).reveal_item_in_dir(path) {
        tracing::warn!(error = %err, "cannot reveal the log file");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_render_process_is_the_one_worth_reloading() {
        assert!(can_reload("render-process-exited"));
        assert!(!can_reload("browser-process-exited"));
    }

    // WebView2 restarts a GPU or utility process on its own and the page lives on. They
    // all read as "other", which was reloaded: a driver reset or waking from sleep threw
    // away the commit message being typed.
    #[test]
    fn a_lost_helper_process_is_not_a_reason_to_reload() {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED,
            COREWEBVIEW2_PROCESS_FAILED_KIND_UTILITY_PROCESS_EXITED,
        };
        for kind in [
            COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED,
            COREWEBVIEW2_PROCESS_FAILED_KIND_UTILITY_PROCESS_EXITED,
        ] {
            assert!(!can_reload(kind_name(kind)), "{}", kind_name(kind));
        }
    }

    #[test]
    fn a_hung_renderer_is_not_reloaded_behind_the_users_back() {
        assert!(is_hung("render-process-unresponsive"));
        assert!(!is_hung("render-process-exited"));
        assert!(!can_reload("render-process-unresponsive"));
    }

    #[test]
    fn the_reason_the_user_reported_has_a_name() {
        assert_eq!(
            reason_name(COREWEBVIEW2_PROCESS_FAILED_REASON_OUT_OF_MEMORY),
            "out-of-memory"
        );
    }

    #[test]
    fn an_unknown_code_from_a_newer_runtime_still_reads_as_something() {
        assert_eq!(
            reason_name(COREWEBVIEW2_PROCESS_FAILED_REASON(9999)),
            "unknown"
        );
        assert_eq!(
            kind_name(COREWEBVIEW2_PROCESS_FAILED_KIND(9999)),
            "other-process-exited"
        );
    }
}
