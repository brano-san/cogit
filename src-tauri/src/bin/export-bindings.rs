fn main() {
    if let Err(err) = cogit_lib::export_bindings() {
        use std::io::Write as _;
        let mut stderr = std::io::stderr();
        let _ = writeln!(stderr, "[cogit] {err:?}");
        std::process::exit(1);
    }
}
