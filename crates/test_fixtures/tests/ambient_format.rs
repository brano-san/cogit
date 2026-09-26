//! A developer's default ref format or hash (and git 3.0's reftable default) must not break
//! the fixtures: their config is written over git's, which held `[extensions]`.
// Its own binary with one test: nothing else reads the environment while it is set.
#![allow(unsafe_code, clippy::unwrap_used)]

#[test]
fn a_default_ref_format_and_hash_from_the_environment_leave_the_fixtures_working() {
    // SAFETY: the only thread of this test binary that touches the environment.
    unsafe {
        std::env::set_var("GIT_DEFAULT_REF_FORMAT", "reftable");
        std::env::set_var("GIT_DEFAULT_HASH", "sha256");
    }

    let f = test_fixtures::linear(2).unwrap();
    f.commit_file(5, "more.txt", "more\n").unwrap();
    f.git(&["switch", "-q", "-c", "topic"]).unwrap();

    assert_eq!(
        f.git(&["rev-parse", "--show-ref-format"]).unwrap().trim(),
        "files"
    );
    assert_eq!(
        f.oid("HEAD").unwrap().len(),
        40,
        "sha1, as gix and every test expect"
    );
}
