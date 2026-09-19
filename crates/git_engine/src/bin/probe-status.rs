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
    use std::io::Write as _;
    let _ = writeln!(std::io::stdout(), "{verdict}");
}
