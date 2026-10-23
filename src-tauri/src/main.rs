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
    // Started by git as its credential helper or by ssh as its `SSH_ASKPASS`: the answer
    // comes from the environment Cogit gave the command, at once and without a window.
    if let Some(code) = git_engine::run_as_helper() {
        std::process::exit(code);
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
    // WebKitGTK workarounds for GPU drivers it trips over, before any thread starts and never
    // over the user's own value. Proprietary NVIDIA on Wayland: explicit sync drops the
    // connection (Error 71); without it DMA-BUF stays zero-copy, 2–4x faster than turning
    // DMA-BUF off. On X11 its GBM buffers fail and the window is black, so DMA-BUF goes
    // (R-764). VMware's vmwgfx: accelerated compositing waits on a fence that never
    // signals, so the window stays black (R-765).
    #[cfg(target_os = "linux")]
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some()
        && !std::env::var("GDK_BACKEND").is_ok_and(|backend| backend.starts_with("x11"));
    #[cfg(target_os = "linux")]
    for (driver, var) in [
        (
            "/proc/driver/nvidia/version",
            if wayland {
                "__NV_DISABLE_EXPLICIT_SYNC"
            } else {
                "WEBKIT_DISABLE_DMABUF_RENDERER"
            },
        ),
        ("/sys/module/vmwgfx", "WEBKIT_DISABLE_COMPOSITING_MODE"),
    ] {
        if std::env::var_os(var).is_none() && std::path::Path::new(driver).exists() {
            // SAFETY: single-threaded here; Tauri and its threads start in `run` below.
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var(var, "1")
            };
        }
    }
    if let Err(err) = cogit_lib::run() {
        cogit_lib::report_fatal(&err);
        std::process::exit(1);
    }
}
