//! Regenerates `frontend/src/lib/ipc/bindings.ts` from the Rust command registry.
//!
//! Run from the repository root:
//!
//! ```sh
//! cargo run -p cogit --bin export-bindings
//! ```
//!
//! The pre-commit hook runs this and rejects the commit if the result differs from what
//! is staged, which is how INV-10 is enforced.

fn main() {
    if let Err(err) = cogit_lib::export_bindings() {
        use std::io::Write as _;
        let mut stderr = std::io::stderr();
        let _ = writeln!(stderr, "[cogit] {err:?}");
        std::process::exit(1);
    }
}
