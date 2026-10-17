#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// `shortDescription` of the bundle: the `Comment=` of the desktop entry.
fn short_description() -> String {
    let conf: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap_or_default();
    conf["bundle"]["shortDescription"]
        .as_str()
        .unwrap_or("Git client")
        .to_owned()
}

fn main() {
    // Started by ssh as its `SSH_ASKPASS` helper (git_engine::pulse::askpass_refusal): there is
    // no one to ask, so the answer is a failure, at once and without a window.
    if std::env::var_os(git_engine::ASKPASS_REFUSAL_VAR).is_some() {
        std::process::exit(1);
    }
    // Before Tauri: these commands need no window, no logging, no webview.
    if let Some(code) = desktop_entry::handle(
        std::env::args_os().skip(1),
        env!("CARGO_PKG_VERSION"),
        &short_description(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    // WebKitGTK's DMA-BUF renderer under the proprietary NVIDIA driver repaints at ~10 fps
    // while a window is resized (R-764). Before any thread starts, and never over the user's own value.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
        && std::path::Path::new("/proc/driver/nvidia/version").exists()
    {
        // SAFETY: single-threaded here; Tauri and its threads start in `run` below.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        };
    }
    if let Err(err) = cogit_lib::run() {
        cogit_lib::report_fatal(&err);
        std::process::exit(1);
    }
}
