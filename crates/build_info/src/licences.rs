//! The Rust half of Help ▸ About ▸ Third-party licences, read from `cargo metadata`.

use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Package {
    pub name: String,
    pub version: String,
    /// The SPDX expression from the manifest.
    pub license: String,
    pub repository: Option<String>,
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn list<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("cargo metadata has no `{key}` list"))
}

/// A dev or build dependency is compiled but not linked into the application.
fn is_normal(dep: &Value) -> bool {
    dep.get("dep_kinds")
        .and_then(Value::as_array)
        .is_some_and(|kinds| kinds.iter().any(|kind| kind["kind"].is_null()))
}

/// Every crate the workspace member `root` links, through normal dependencies, with
/// the workspace's own crates left out. Sorted by name, ignoring case.
pub fn shipped(metadata: &Value, root: &str) -> Result<Vec<Package>, String> {
    let packages = list(metadata, "packages")?;
    let members: HashSet<&str> = list(metadata, "workspace_members")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let nodes: HashMap<&str, &Value> =
        list(metadata.get("resolve").unwrap_or(&Value::Null), "nodes")?
            .iter()
            .filter_map(|node| Some((text(node, "id")?, node)))
            .collect();

    let start = packages
        .iter()
        .find(|package| {
            text(package, "name") == Some(root)
                && text(package, "id").is_some_and(|id| members.contains(id))
        })
        .and_then(|package| text(package, "id"))
        .ok_or_else(|| format!("`{root}` is not a workspace member"))?;

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(id) = queue.pop_front() {
        let deps = nodes
            .get(id)
            .and_then(|node| node.get("deps"))
            .and_then(Value::as_array);
        for dep in deps.into_iter().flatten().filter(|dep| is_normal(dep)) {
            if let Some(next) = text(dep, "pkg")
                && seen.insert(next)
            {
                queue.push_back(next);
            }
        }
    }

    let mut found: Vec<Package> = packages
        .iter()
        .filter(|package| {
            text(package, "id").is_some_and(|id| seen.contains(id) && !members.contains(id))
        })
        .map(|package| Package {
            name: text(package, "name").unwrap_or("?").to_owned(),
            version: text(package, "version").unwrap_or("?").to_owned(),
            license: match (text(package, "license"), text(package, "license_file")) {
                (Some(expression), _) => expression.to_owned(),
                (None, Some(file)) => format!("see {file}"),
                (None, None) => "unknown".to_owned(),
            },
            repository: text(package, "repository").map(str::to_owned),
        })
        .collect();
    found.sort_by_cached_key(|package| (package.name.to_lowercase(), package.version.clone()));
    Ok(found)
}

#[must_use]
pub fn render(packages: &[Package]) -> String {
    let mut out = format!("Rust crates ({})\n\n", packages.len());
    for package in packages {
        out.push_str(&format!(
            "{} {} — {}",
            package.name, package.version, package.license
        ));
        if let Some(repository) = &package.repository {
            out.push_str(&format!(" — {repository}"));
        }
        out.push('\n');
    }
    out
}
