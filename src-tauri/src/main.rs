#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(err) = cogit_lib::run() {
        use std::io::Write as _;
        let mut stderr = std::io::stderr();
        let _ = writeln!(stderr, "[cogit] fatal: {err:?}");
        std::process::exit(1);
    }
}
