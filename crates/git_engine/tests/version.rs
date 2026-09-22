//! The About window reports which `git` is actually being driven; nothing outside
//! `git_engine` may ask, so the answer has to come from here.

use git_engine::git_version;

#[test]
fn the_version_of_the_git_that_will_be_run_is_reported() {
    let version = git_version().expect("the test machine has git on PATH");
    assert!(
        version.starts_with("git version "),
        "unexpected answer: {version}"
    );
}

#[test]
fn the_answer_has_no_trailing_newline_to_paste_into_a_report() {
    let version = git_version().expect("the test machine has git on PATH");
    assert_eq!(version.trim_end(), version);
}

#[test]
fn the_git_library_reports_its_own_version_as_plain_numbers() {
    let version = git_engine::gix_version();
    let parts: Vec<&str> = version.split('.').collect();
    assert_eq!(parts.len(), 3, "unexpected answer: {version}");
    assert!(
        parts.iter().all(|part| part.parse::<u32>().is_ok()),
        "unexpected answer: {version}"
    );
}
