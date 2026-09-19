//! Prints `clean` or `dirty` for the repository at the given path.
//!
//! Exists so a test can run it as a child process with a hostile `GIT_*` environment:
//! `std::env::set_var` is unsafe in edition 2024 and races with parallel tests.

fn main() {
    let mut args = std::env::args_os().skip(1);
    let Some(path) = args.next() else {
        std::process::exit(2);
    };
    let repo = match git_engine::RepoHandle::open(std::path::Path::new(&path)) {
        Ok(repo) => repo,
        Err(_) => std::process::exit(3),
    };
    let status = match repo.status() {
        Ok(status) => status,
        Err(_) => std::process::exit(4),
    };
    let verdict = if status.is_clean() { "clean" } else { "dirty" };
    // A probe binary exists to print; `tracing` has no subscriber here.
    use std::io::Write as _;
    let _ = writeln!(std::io::stdout(), "{verdict}");
}
