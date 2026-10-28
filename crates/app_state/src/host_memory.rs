use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub(crate) struct HostMemory {
    file: Option<PathBuf>,
    used: BTreeSet<String>,
}

impl HostMemory {
    pub(crate) fn begin(&mut self, file: PathBuf) -> Vec<String> {
        let last: Vec<String> = std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        self.file = Some(file);
        self.used.clear();
        self.save();
        last
    }

    pub(crate) fn remember(&mut self, distro: &str) {
        if self.file.is_some() && self.used.insert(distro.to_owned()) {
            self.save();
        }
    }

    fn save(&self) {
        let Some(file) = &self.file else { return };
        if let Err(err) = write(file, &self.used) {
            tracing::error!(error = ?err, context = "the WSL distributions used this run");
        }
    }
}

fn write(file: &Path, used: &BTreeSet<String>) -> std::io::Result<()> {
    std::fs::write(file, serde_json::to_vec(used)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_starts_the_distributions_the_last_one_used_and_forgets_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("wsl.json");
        std::fs::write(&file, br#"["Ubuntu","Debian"]"#).unwrap();

        let mut memory = HostMemory::default();
        assert_eq!(memory.begin(file.clone()), ["Ubuntu", "Debian"]);
        memory.remember("Ubuntu");
        memory.remember("Ubuntu");

        assert_eq!(HostMemory::default().begin(file), ["Ubuntu"]);
    }

    #[test]
    fn no_file_starts_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            HostMemory::default()
                .begin(dir.path().join("wsl.json"))
                .is_empty()
        );
    }
}
