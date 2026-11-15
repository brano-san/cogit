use git_engine::{RepoHandle, ResetMode};
use std::time::Instant;

#[test]
fn config_and_reset_do_not_spawn_read_commands() {
    use std::sync::{Arc, Mutex};
    let f = test_fixtures::linear(3).unwrap();
    f.git(&["tag", "-a", "before", "HEAD~1", "-m", "before"])
        .unwrap();
    let commands = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&commands);
    let repo = RepoHandle::open(f.path())
        .unwrap()
        .with_journal(Arc::new(move |out| {
            sink.lock().unwrap().push(out.command);
        }));
    assert!(repo.config_path().is_file());
    repo.reset("before", ResetMode::Soft).unwrap();
    assert_eq!(f.oid("HEAD").unwrap(), f.oid("before^{commit}").unwrap());
    repo.reset("HEAD~1", ResetMode::Soft).unwrap();
    let commands = commands.lock().unwrap();
    assert_eq!(commands.len(), 2, "{commands:?}");
    assert!(
        commands
            .iter()
            .all(|command| command.starts_with("git reset --soft "))
    );
}

#[test]
fn an_invalid_reset_keeps_the_git_diagnostic() {
    let f = test_fixtures::linear(1).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    let before = f.oid("HEAD").unwrap();
    let error = repo.reset("missing-revision", ResetMode::Soft).unwrap_err();
    assert!(matches!(error, git_engine::GitError::Command(_)));
    assert_eq!(f.oid("HEAD").unwrap(), before);
}

#[test]
#[ignore = "manual timing; run alone with --ignored --nocapture"]
#[allow(clippy::print_stderr)]
fn read_overhead() {
    let f = test_fixtures::linear(3).unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();
    for (label, reset) in [("config", false), ("reset", true)] {
        let started = Instant::now();
        for _ in 0..20 {
            if reset {
                repo.reset("HEAD", ResetMode::Soft).unwrap();
            } else {
                std::hint::black_box(repo.config_path());
            }
        }
        eprintln!("{label}: {:?}/call", started.elapsed() / 20);
    }
}
