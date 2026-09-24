use git_engine::redact_command;

#[test]
fn an_ordinary_command_is_untouched() {
    assert_eq!(
        redact_command(&["status", "--porcelain"]),
        "git status --porcelain"
    );
}

#[test]
fn an_authorization_header_is_hidden() {
    let line = redact_command(&[
        "-c",
        "http.extraHeader=Authorization: Basic c2VjcmV0",
        "push",
    ]);

    assert!(!line.contains("c2VjcmV0"), "{line}");
    assert!(line.contains("http.extraHeader="), "{line}");
}

#[test]
fn a_password_inside_a_remote_url_is_hidden() {
    let line = redact_command(&["push", "https://user:ghp_deadbeef@github.com/o/r.git"]);

    assert!(!line.contains("ghp_deadbeef"), "{line}");
    assert!(line.contains("github.com/o/r.git"), "{line}");
    assert!(line.contains("user"), "{line}");
}

#[test]
fn a_url_without_credentials_is_left_alone() {
    let url = "https://github.com/o/r.git";
    assert!(redact_command(&["push", url]).contains(url));
}

#[test]
fn a_colon_in_a_refspec_is_not_mistaken_for_a_password() {
    let line = redact_command(&["push", "origin", "HEAD:refs/heads/main"]);
    assert!(line.contains("HEAD:refs/heads/main"), "{line}");
}

#[test]
fn an_ssh_url_with_a_colon_is_left_alone() {
    let url = "git@github.com:owner/repo.git";
    assert!(redact_command(&["remote", "add", "origin", url]).contains(url));
}

#[test]
fn the_auth_header_is_basic_and_carries_the_token() {
    let header = git_engine::auth_header("s3cr3t");

    assert!(header.starts_with("Authorization: Basic "), "{header}");
    assert!(
        !header.contains("s3cr3t"),
        "the token must be encoded, not literal: {header}"
    );
}

#[test]
fn the_auth_header_encodes_the_documented_user_and_token() {
    // base64("x-access-token:abc")
    assert_eq!(
        git_engine::auth_header("abc"),
        "Authorization: Basic eC1hY2Nlc3MtdG9rZW46YWJj"
    );
}

#[test]
fn the_auth_header_never_reaches_the_journal_line() {
    let header = git_engine::auth_header("abc");
    let arg = format!("http.extraHeader={header}");
    let line = redact_command(&["-c", &arg, "push"]);

    assert!(!line.contains("eC1hY2Nlc3M"), "{line}");
}

#[test]
fn only_an_https_remote_gets_a_token() {
    assert!(git_engine::wants_auth("https://github.com/o/r.git"));
    assert!(!git_engine::wants_auth("git@github.com:o/r.git"));
    assert!(!git_engine::wants_auth("ssh://git@github.com/o/r.git"));
    assert!(!git_engine::wants_auth("/srv/git/r.git"));
}

// Redaction cut at the first `@`, so a password containing one kept its tail in the
// journal, the log file and the error dialog.
#[test]
fn a_password_with_an_at_sign_is_hidden_whole() {
    let line = redact_command(&["push", "https://user:p@ss@github.com/o/r.git"]);

    assert!(!line.contains("ss@"), "{line}");
    assert!(line.contains("user:"), "{line}");
    assert!(line.contains("@github.com/o/r.git"), "{line}");
}

#[test]
fn a_password_with_an_at_sign_is_hidden_whole_in_git_output_too() {
    let text = git_engine::output_text::redact_secrets(
        "fatal: unable to access 'https://user:p@ss@github.com/o/r.git/'",
    );

    assert!(!text.contains("ss@"), "{text}");
    assert!(text.contains("@github.com/o/r.git/"), "{text}");
}
