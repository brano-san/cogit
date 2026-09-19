// Hides the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(err) = cogit_lib::run() {
        // `tracing` may not be installed yet if startup failed early, so write directly.
        use std::io::Write as _;
        let mut stderr = std::io::stderr();
        let _ = writeln!(stderr, "[cogit] fatal: {err:?}");
        std::process::exit(1);
    }
}
