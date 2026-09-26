//! Hooks and the preset catalogue: reading them, writing them, running one by hand, and
//! the record of every time somebody went round them.

use crate::{AppState, PresetStatus, RepoId, presets};

impl AppState {
    pub fn hooks(&self, repo: RepoId) -> Result<git_engine::HookOverview, git_engine::GitError> {
        self.handle(repo)?.hooks()
    }

    pub fn read_hook(&self, repo: RepoId, name: &str) -> Result<String, git_engine::GitError> {
        self.handle(repo)?.read_hook(name)
    }

    pub fn write_hook(
        &self,
        repo: RepoId,
        name: &str,
        body: &str,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.write_hook(name, body)
    }

    pub fn set_hook_enabled(
        &self,
        repo: RepoId,
        name: &str,
        enabled: bool,
    ) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?.set_hook_enabled(name, enabled)
    }

    /// Wires a versioned hook directory up. Never automatic: a hooks path inside the tree
    /// turns repository content into code that runs on commit (doc/modules/M10-hooks.md).
    pub fn use_hooks_path(&self, repo: RepoId, path: &str) -> Result<(), git_engine::GitError> {
        let _quiet = self.quiet(repo);
        self.handle(repo)?
            .run_git(&["config", "core.hooksPath", path])
            .map(drop)
    }

    /// Wiring up a team's hooks is two steps, and the second is the one people forget.
    pub fn adopt_hooks(&self, repo: RepoId, path: &str) -> Result<(), git_engine::GitError> {
        self.use_hooks_path(repo, path)?;
        self.handle(repo)?.add_eol_rule(path)
    }

    pub fn run_hook(
        &self,
        repo: RepoId,
        name: &str,
    ) -> Result<git_engine::HookRun, git_engine::GitError> {
        self.handle(repo)?.run_hook(name)
    }

    /// A failing check is a verdict the user reads, so it is tracked like any other run
    /// and never turned into an error that stops the rebase (T11.3).
    pub fn run_check(
        &self,
        repo: RepoId,
        command: &str,
    ) -> Result<git_engine::HookRun, git_engine::GitError> {
        self.handle(repo)?.run_check(command)
    }

    pub fn bypass_log(
        &self,
        repo: RepoId,
    ) -> Result<Vec<git_engine::Bypass>, git_engine::GitError> {
        self.handle(repo)?.bypass_log()
    }

    pub fn commit_template(&self, repo: RepoId) -> Result<Option<String>, git_engine::GitError> {
        self.handle(repo)?.commit_template()
    }

    /// Where the user's own presets live. Set once at startup from the app config dir.
    pub fn use_preset_dir(&self, dir: std::path::PathBuf) {
        *self.preset_dir.write() = Some(dir);
    }

    /// The catalogue told against one repository: tools resolved and config files checked.
    pub fn presets_for(&self, repo: RepoId) -> Result<Vec<PresetStatus>, git_engine::GitError> {
        let root = self.get(repo).ok_or_else(|| crate::not_open(repo))?.root;

        let mut all: Vec<PresetStatus> = git_engine::builtin_presets()
            .into_iter()
            .map(|preset| presets::status_for(preset, &root, false))
            .collect();
        if let Some(dir) = self.preset_dir.read().clone() {
            all.extend(
                presets::user_presets(&dir)
                    .into_iter()
                    .map(|preset| presets::status_for(preset, &root, true)),
            );
        }
        Ok(all)
    }

    /// Saves the hook as it stands now as a preset the user can install elsewhere.
    pub fn export_preset(
        &self,
        repo: RepoId,
        hook: &str,
        id: &str,
        name: &str,
        description: &str,
    ) -> Result<(), git_engine::GitError> {
        if !presets::valid_id(id) {
            return Err(git_engine::GitError::InvalidState(format!(
                "{id} is not a usable preset name"
            )));
        }
        if git_engine::builtin_presets().iter().any(|p| p.id == id) {
            return Err(git_engine::GitError::InvalidState(format!(
                "{id} is the name of a built-in preset"
            )));
        }
        let dir = self.preset_dir()?;
        let script = self.handle(repo)?.read_hook(hook)?;
        presets::write_preset(&dir, id, name, hook, description, &script).map(drop)
    }

    pub fn remove_preset(&self, id: &str) -> Result<(), git_engine::GitError> {
        if !presets::valid_id(id) || git_engine::builtin_presets().iter().any(|p| p.id == id) {
            return Err(git_engine::GitError::InvalidState(format!(
                "{id} is not a preset of yours"
            )));
        }
        std::fs::remove_file(self.preset_dir()?.join(format!("{id}.toml")))?;
        Ok(())
    }

    fn preset_dir(&self) -> Result<std::path::PathBuf, git_engine::GitError> {
        self.preset_dir.read().clone().ok_or_else(|| {
            git_engine::GitError::InvalidState("no directory for your own presets".into())
        })
    }

    pub fn install_preset(&self, repo: RepoId, id: &str) -> Result<(), git_engine::GitError> {
        let saved = self
            .preset_dir
            .read()
            .clone()
            .map(|dir| presets::user_presets(&dir))
            .unwrap_or_default();
        let preset = git_engine::builtin_presets()
            .into_iter()
            .chain(saved)
            .find(|preset| preset.id == id)
            .ok_or_else(|| {
                git_engine::GitError::InvalidState(format!("there is no preset {id}"))
            })?;
        let _quiet = self.quiet(repo);
        self.handle(repo)?.install_preset(&preset)
    }
}
