#![allow(clippy::unwrap_used, clippy::expect_used)]

use hosting::{HostingAccount, HostingProvider, RemoteRef, account_for};

fn account(
    id: &str,
    provider: HostingProvider,
    server: Option<&str>,
    login: &str,
) -> HostingAccount {
    HostingAccount {
        id: id.into(),
        provider,
        server_url: server.map(str::to_owned),
        login: login.into(),
        username: None,
        use_for_git: true,
    }
}

fn chosen(accounts: &[HostingAccount], url: &str) -> Option<(String, String)> {
    account_for(accounts, url).map(|found| (found.account.id, found.repository))
}

#[test]
fn every_remote_form_reaches_the_same_repository() {
    let accounts = [account("gh", HostingProvider::Github, None, "ann")];
    for url in [
        "https://github.com/acme/app.git",
        "https://github.com/acme/app",
        "https://ann:tok@github.com/acme/app.git/",
        "git@github.com:acme/app.git",
        "ssh://git@github.com/acme/app.git",
        "ssh://git@GitHub.com:22/acme/app",
        "git+ssh://git@github.com/acme/app.git",
    ] {
        assert_eq!(
            chosen(&accounts, url),
            Some(("gh".into(), "acme/app".into())),
            "{url}"
        );
    }
}

#[test]
fn no_account_for_another_host_or_a_path() {
    let accounts = [account("gh", HostingProvider::Github, None, "ann")];
    assert_eq!(chosen(&accounts, "https://gitlab.com/acme/app.git"), None);
    assert_eq!(chosen(&accounts, "C:/work/app"), None);
    assert_eq!(chosen(&accounts, "/srv/git/app.git"), None);
    assert_eq!(chosen(&accounts, "https://github.com/"), None);
}

#[test]
fn of_several_accounts_on_one_host_the_url_user_or_the_owner_wins() {
    let accounts = [
        account("work", HostingProvider::Github, None, "ann-work"),
        account("home", HostingProvider::Github, None, "Ann"),
    ];
    let id = |url| chosen(&accounts, url).unwrap().0;
    assert_eq!(id("git@github.com:acme/app.git"), "work");
    assert_eq!(id("git@github.com:ann/dotfiles.git"), "home");
    assert_eq!(id("https://ann@github.com/acme/app.git"), "home");
    assert_eq!(id("https://ANN-WORK@github.com/ann/x.git"), "work");
}

#[test]
fn a_self_hosted_server_under_a_path_matches_https_inside_it_and_ssh_by_host() {
    let accounts = [
        account("cloud", HostingProvider::Gitlab, None, "ann"),
        account(
            "corp",
            HostingProvider::Gitlab,
            Some("https://git.corp.example/gitlab"),
            "ann",
        ),
        account(
            "ghe",
            HostingProvider::Github,
            Some("https://ghe.corp.example"),
            "ann",
        ),
    ];
    assert_eq!(
        chosen(
            &accounts,
            "https://git.corp.example/gitlab/team/sub/app.git"
        ),
        Some(("corp".into(), "team/sub/app".into()))
    );
    assert_eq!(
        chosen(&accounts, "https://git.corp.example/other/app.git"),
        None
    );
    assert_eq!(
        chosen(&accounts, "git@git.corp.example:team/app.git"),
        Some(("corp".into(), "team/app".into()))
    );
    assert_eq!(
        chosen(&accounts, "https://gitlab.com/team/app.git"),
        Some(("cloud".into(), "team/app".into()))
    );
    let found = account_for(&accounts, "git@ghe.corp.example:acme/app.git").unwrap();
    assert_eq!(found.api_url, "https://ghe.corp.example/api/v3");
    assert_eq!(found.web_url, "https://ghe.corp.example/acme/app");
}

#[test]
fn remote_parts() {
    assert_eq!(
        RemoteRef::parse("https://u:p@Host.example:8443/a/b.git"),
        Some(RemoteRef {
            host: "host.example".into(),
            path: "a/b".into(),
            user: Some("u".into()),
            https: true,
        })
    );
    assert_eq!(RemoteRef::parse("file:///srv/app.git"), None);
    assert_eq!(RemoteRef::parse("../app"), None);
}
