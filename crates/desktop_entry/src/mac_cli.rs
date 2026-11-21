use std::io;
use std::path::Path;

const EXECUTABLE: &str = "/Applications/Cogit.app/Contents/MacOS/cogit";
const LINK: &str = "/usr/local/bin/cogit";

#[cfg(target_os = "macos")]
pub fn enter_bundle() -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    let started = std::env::current_exe()?;
    if started == Path::new(LINK)
        && std::fs::read_link(&started).is_ok_and(|target| target == Path::new(EXECUTABLE))
    {
        return Err(std::process::Command::new(EXECUTABLE)
            .args(std::env::args_os().skip(1))
            .exec());
    }
    Ok(())
}

fn needs_install(exe: &Path, link: &Path) -> io::Result<bool> {
    if exe != Path::new(EXECUTABLE) {
        return Err(io::Error::other(
            "Move Cogit.app to /Applications and open it there first.",
        ));
    }
    match std::fs::symlink_metadata(link) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(err) => Err(err),
        Ok(meta) if meta.is_symlink() && std::fs::read_link(link)? == exe => Ok(false),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists.", link.display()),
        )),
    }
}

pub fn install() -> io::Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Installing this link requires macOS.",
        ));
    }
    let exe = std::env::current_exe()?;
    let link = Path::new(LINK);
    if needs_install(&exe, link)? {
        let script = format!(
            "do shell script \"/bin/mkdir -p /usr/local/bin && /bin/ln -s {EXECUTABLE} /usr/local/bin/\" with administrator privileges"
        );
        let output = std::process::Command::new("/usr/bin/osascript")
            .args(["-e", &script])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_installed_bundle_can_create_a_missing_link() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("cogit");
        assert!(
            needs_install(
                Path::new("/Volumes/Cogit/Cogit.app/Contents/MacOS/cogit"),
                &link
            )
            .is_err()
        );
        assert!(needs_install(Path::new(EXECUTABLE), &link).unwrap());
        std::fs::write(&link, "mine").unwrap();
        assert_eq!(
            needs_install(Path::new(EXECUTABLE), &link)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
    }

    #[cfg(unix)]
    #[test]
    fn installing_twice_accepts_only_our_own_link() {
        let dir = tempfile::tempdir().unwrap();
        let ours = dir.path().join("ours");
        let other = dir.path().join("other");
        std::os::unix::fs::symlink(EXECUTABLE, &ours).unwrap();
        std::os::unix::fs::symlink("/missing", &other).unwrap();
        assert!(!needs_install(Path::new(EXECUTABLE), &ours).unwrap());
        assert_eq!(
            needs_install(Path::new(EXECUTABLE), &other)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
    }
}
