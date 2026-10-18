// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! `i18n.logOutputEncoding` (Repository ▸ Settings offers it) re-encodes what `log` prints,
//! and every parsed read decodes UTF-8: a Cyrillic subject came back as U+FFFD, and an
//! interactive rebase wrote that over the message of an untouched Squash.

use git_engine::RepoHandle;

#[test]
fn a_subject_reads_back_whatever_the_log_output_encoding() {
    let f = test_fixtures::linear(1).unwrap();
    f.write_file("ru.txt", "x\n").unwrap();
    f.git(&["add", "--", "ru.txt"]).unwrap();
    f.git(&["commit", "-q", "-m", "Исправить сборку"]).unwrap();
    f.git(&["config", "i18n.logOutputEncoding", "cp1251"])
        .unwrap();
    let repo = RepoHandle::open(f.path()).unwrap();

    let plan = repo.rebase_todo(&f.oid("HEAD~1").unwrap()).unwrap();

    assert_eq!(plan[0].message.as_deref(), Some("Исправить сборку"));
}
