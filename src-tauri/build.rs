use std::process::Command;

/// Stamps the build with what a bug report needs: which commit, built when, by which
/// compiler. `git` may be absent in a clean-checkout build, so a miss is "unknown", not
/// a failed build.
fn main() {
    let commit = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |out| out.trim().to_owned());

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
    println!("cargo:rustc-env=COGIT_BUILT_AT={built}");
    println!("cargo:rustc-env=COGIT_RUSTC={rustc}");
    // HEAD holds `ref: refs/heads/master`, which a commit does not change; the branch
    // file, or packed-refs once git packs it, is what moves.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/packed-refs");
    if let Some(branch) = std::fs::read_to_string("../.git/HEAD")
        .ok()
        .and_then(|head| {
            head.strip_prefix("ref: ")
                .map(|name| name.trim().to_owned())
        })
    {
        println!("cargo:rerun-if-changed=../.git/{branch}");
    }

    tauri_build::build();
}
