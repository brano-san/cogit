//! Test seam: dry-runs one hook of the repository at the path given and prints what it
//! wrote, so a test can start it with an environment of its own (R-22).
fn main() {
    let mut args = std::env::args_os().skip(1);
    let (Some(path), Some(hook)) = (args.next(), args.next()) else {
        std::process::exit(2);
    };
    let Ok(repo) = git_engine::RepoHandle::open(std::path::Path::new(&path)) else {
        std::process::exit(3);
    };
    let Ok(run) = repo.run_hook(&hook.to_string_lossy()) else {
        std::process::exit(4);
    };
    use std::io::Write as _;
    let _ = write!(std::io::stdout(), "{}", run.stdout);
}
