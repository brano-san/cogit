// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use build_info::licences::{Package, render, shipped};
use serde_json::{Value, json};

const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

fn id(name: &str) -> String {
    format!("{REGISTRY}#{name}@1.0.0")
}

fn member(name: &str) -> String {
    format!("path+file:///work/{name}#0.1.0")
}

fn package(name: &str, license: Option<&str>) -> Value {
    json!({
        "id": id(name),
        "name": name,
        "version": "1.0.0",
        "license": license,
        "license_file": null,
        "repository": format!("https://example.test/{name}"),
        "source": REGISTRY,
    })
}

fn dep(pkg: &str, kind: Option<&str>) -> Value {
    json!({ "name": pkg, "pkg": pkg, "dep_kinds": [{ "kind": kind, "target": null }] })
}

/// `app` links `lib` (which links `leaf`) and `own`, a workspace crate; `tool` is a
/// build dependency and `checker` a dev dependency, and neither ships.
fn metadata() -> Value {
    json!({
        "packages": [
            { "id": member("app"), "name": "app", "version": "0.1.0", "license": "MIT",
              "license_file": null, "repository": null, "source": null },
            { "id": member("own"), "name": "own", "version": "0.1.0", "license": "MIT",
              "license_file": null, "repository": null, "source": null },
            package("lib", Some("MIT OR Apache-2.0")),
            package("leaf", Some("Zlib")),
            package("tool", Some("MIT")),
            package("checker", Some("MIT")),
            package("unrelated", Some("MIT")),
        ],
        "workspace_members": [member("app"), member("own")],
        "resolve": { "nodes": [
            { "id": member("app"), "deps": [
                dep(&id("lib"), None),
                dep(&member("own"), None),
                dep(&id("tool"), Some("build")),
                dep(&id("checker"), Some("dev")),
            ]},
            { "id": member("own"), "deps": [dep(&id("leaf"), None)] },
            { "id": id("lib"), "deps": [dep(&id("leaf"), None)] },
            { "id": id("leaf"), "deps": [] },
            { "id": id("tool"), "deps": [] },
            { "id": id("checker"), "deps": [] },
            { "id": id("unrelated"), "deps": [] },
        ]},
    })
}

fn names(packages: &[Package]) -> Vec<&str> {
    packages
        .iter()
        .map(|package| package.name.as_str())
        .collect()
}

#[test]
fn only_what_the_application_links_is_listed() {
    let packages = shipped(&metadata(), "app").unwrap();
    assert_eq!(names(&packages), ["leaf", "lib"]);
}

#[test]
fn the_list_is_alphabetical_whatever_the_case_of_a_name() {
    let mut metadata = metadata();
    metadata["packages"][2]["name"] = json!("Lib");
    let packages = shipped(&metadata, "app").unwrap();
    assert_eq!(names(&packages), ["leaf", "Lib"]);
}

#[test]
fn the_licence_expression_and_the_source_travel_with_each_crate() {
    let packages = shipped(&metadata(), "app").unwrap();
    let lib = packages
        .iter()
        .find(|package| package.name == "lib")
        .unwrap();
    assert_eq!(lib.version, "1.0.0");
    assert_eq!(lib.license, "MIT OR Apache-2.0");
    assert_eq!(lib.repository.as_deref(), Some("https://example.test/lib"));
}

#[test]
fn a_crate_without_a_licence_field_is_listed_as_unknown_rather_than_dropped() {
    let mut metadata = metadata();
    metadata["packages"][3]["license"] = Value::Null;
    let packages = shipped(&metadata, "app").unwrap();
    let leaf = packages
        .iter()
        .find(|package| package.name == "leaf")
        .unwrap();
    assert_eq!(leaf.license, "unknown");
}

#[test]
fn a_crate_with_only_a_licence_file_points_at_it() {
    let mut metadata = metadata();
    metadata["packages"][3]["license"] = Value::Null;
    metadata["packages"][3]["license_file"] = json!("LICENSE.txt");
    let packages = shipped(&metadata, "app").unwrap();
    let leaf = packages
        .iter()
        .find(|package| package.name == "leaf")
        .unwrap();
    assert_eq!(leaf.license, "see LICENSE.txt");
}

#[test]
fn an_unknown_root_is_an_error_not_an_empty_list() {
    assert!(shipped(&metadata(), "missing").is_err());
}

#[test]
fn metadata_of_the_wrong_shape_is_an_error() {
    assert!(shipped(&json!({ "packages": 3 }), "app").is_err());
}

#[test]
fn the_rendered_list_has_one_line_per_crate_under_a_count() {
    let packages = shipped(&metadata(), "app").unwrap();
    let text = render(&packages);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "Rust crates (2)");
    assert!(lines.contains(&"leaf 1.0.0 — Zlib — https://example.test/leaf"));
    assert!(lines.contains(&"lib 1.0.0 — MIT OR Apache-2.0 — https://example.test/lib"));
}

#[test]
fn a_crate_without_a_repository_has_no_dangling_separator() {
    let packages = [Package {
        name: "bare".to_owned(),
        version: "0.1.0".to_owned(),
        license: "MIT".to_owned(),
        repository: None,
    }];
    assert!(
        render(&packages)
            .lines()
            .any(|line| line == "bare 0.1.0 — MIT")
    );
}
