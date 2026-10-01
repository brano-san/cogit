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
    if let Err(err) = cogit_lib::run() {
        use std::io::Write as _;
        let mut stderr = std::io::stderr();
        let _ = writeln!(stderr, "[cogit] fatal: {err:?}");
        std::process::exit(1);
    }
}
