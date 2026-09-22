use std::hash::{Hash as _, Hasher as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Docs do not make a binary differ from its commit: neither watched nor counted as dirty.
const CODE: [&str; 5] = [
    ":/src-tauri",
    ":/crates",
    ":/frontend",
    ":/Cargo.toml",
    ":/Cargo.lock",
];

fn git(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|out| out.trim().to_owned())
}

/// Without `git` at build time the stamp says `unknown`; the build itself goes on.
fn main() {
    let commit = git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    // `--no-optional-locks`: status must not rewrite the index of the tree it inspects.
    let mut status = vec![
        "--no-optional-locks",
        "status",
        "--porcelain",
        "--untracked-files=no",
        "--",
    ];
    status.extend(CODE);
    let dirty = git(&status).is_some_and(|changes| !changes.is_empty());

    let built = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|epoch| epoch.parse::<i64>().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_secs() as i64)
        });

    let rustc = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned()))
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |out| out.trim().to_owned());

    println!("cargo:rustc-env=COGIT_COMMIT={commit}");
    println!("cargo:rustc-env=COGIT_DIRTY={dirty}");
    println!("cargo:rustc-env=COGIT_BUILT_AT={built}");
    println!("cargo:rustc-env=COGIT_RUSTC={rustc}");

    watch_git_state();
    watch_code();
    if let Some(out_dir) = std::env::var_os("OUT_DIR") {
        write_crate_list(Path::new(&out_dir));
    }

    tauri_build::build();
}

/// A commit moves the branch file, not HEAD (R-156); a missing path would rerun every build.
fn watch_git_state() {
    let mut names = vec!["HEAD".to_owned(), "packed-refs".to_owned()];
    names.extend(git(&["symbolic-ref", "-q", "HEAD"]));
    for name in names {
        if let Some(path) = git(&["rev-parse", "--git-path", &name])
            && Path::new(&path).exists()
        {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}

/// Rust sources rebuild this crate anyway; the frontend only in release, which re-embeds it.
fn watch_code() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=../Cargo.lock");
    if let Ok(crates) = std::fs::read_dir("../crates") {
        for entry in crates.filter_map(Result::ok) {
            let src = entry.path().join("src");
            if src.exists() {
                println!("cargo:rerun-if-changed={}", src.display());
            }
        }
    }
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        println!("cargo:rerun-if-changed=../frontend/src");
    }
}

/// `cargo metadata` takes seconds, so its answer is kept until `Cargo.lock` changes.
fn write_crate_list(out_dir: &Path) {
    let list = out_dir.join("third-party-crates.txt");
    let stamp_file = out_dir.join("third-party-crates.stamp");

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::fs::read("../Cargo.lock")
        .unwrap_or_default()
        .hash(&mut hasher);
    let stamp = format!("{:x}", hasher.finish());
    if list.exists() && std::fs::read_to_string(&stamp_file).is_ok_and(|kept| kept == stamp) {
        return;
    }

    let (text, fresh) = match crate_list() {
        Ok(text) => (text, true),
        Err(err) => {
            println!("cargo:warning=third-party crate list not generated: {err}");
            (
                format!("Rust crates\n\nThe list could not be generated for this build: {err}\n"),
                false,
            )
        }
    };
    let _ = std::fs::write(&list, text);
    if fresh {
        let _ = std::fs::write(&stamp_file, stamp);
    }
}

/// Offline first; a machine without other platforms' manifests needs the registry once.
fn metadata(target: &str) -> Result<Vec<u8>, String> {
    let cargo = std::env::var_os("CARGO").map_or_else(|| PathBuf::from("cargo"), PathBuf::from);
    let run = |offline: bool| {
        let mut command = Command::new(&cargo);
        command.args(["metadata", "--format-version", "1", "--locked"]);
        command.args(["--filter-platform", target]);
        if offline {
            command.arg("--offline");
        }
        build_info::process::output_within(command, Duration::from_secs(120))
    };
    run(true)
        .or_else(|_| run(false))
        .map_err(|err| format!("cargo metadata: {err}"))
}

fn crate_list() -> Result<String, String> {
    let target = std::env::var("TARGET").map_err(|err| format!("TARGET: {err}"))?;
    let out = metadata(&target)?;
    let metadata: serde_json::Value =
        serde_json::from_slice(&out).map_err(|err| format!("cargo metadata output: {err}"))?;
    let packages = build_info::licences::shipped(&metadata, "cogit")?;
    Ok(build_info::licences::render(&packages))
}
