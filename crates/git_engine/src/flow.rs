//! Git-Flow over ordinary git commands (M5 T5.5). The `git-flow` extension is not a
//! dependency: it is a shell script around branch, merge and tag, and shelling out to
//! something the user may not have installed is worse than doing the three steps here.
//! The config keys are the extension's own, so a repository set up either way is read
//! correctly by both.

use crate::{GitError, RepoHandle, Result};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlowConfig {
    pub main: String,
    pub develop: String,
    pub feature: String,
    pub release: String,
    pub hotfix: String,
}

impl Default for FlowConfig {
    fn default() -> Self {
        Self {
            main: "main".to_owned(),
            develop: "develop".to_owned(),
            feature: "feature/".to_owned(),
            release: "release/".to_owned(),
            hotfix: "hotfix/".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FlowKind {
    Feature,
    Release,
    Hotfix,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlowBranch {
    pub kind: FlowKind,
    /// Without the prefix, which is what the user typed when starting it.
    pub name: String,
    pub full: String,
    pub is_head: bool,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlowStatus {
    pub initialised: bool,
    pub config: FlowConfig,
    pub branches: Vec<FlowBranch>,
}

impl FlowConfig {
    fn prefix(&self, kind: FlowKind) -> &str {
        match kind {
            FlowKind::Feature => &self.feature,
            FlowKind::Release => &self.release,
            FlowKind::Hotfix => &self.hotfix,
        }
    }

    /// Where the branch is cut from: a hotfix patches what is released, not what is next.
    fn base(&self, kind: FlowKind) -> &str {
        match kind {
            FlowKind::Feature | FlowKind::Release => &self.develop,
            FlowKind::Hotfix => &self.main,
        }
    }
}

type ConfigKey = (&'static str, fn(&FlowConfig) -> &str);

const KEYS: &[ConfigKey] = &[
    ("gitflow.branch.master", |c| &c.main),
    ("gitflow.branch.develop", |c| &c.develop),
    ("gitflow.prefix.feature", |c| &c.feature),
    ("gitflow.prefix.release", |c| &c.release),
    ("gitflow.prefix.hotfix", |c| &c.hotfix),
];

impl RepoHandle {
    pub fn flow_status(&self) -> Result<FlowStatus> {
        let snapshot = self.repo.config_snapshot();
        let read = |key: &str| snapshot.string(key).map(|value| value.to_string());
        let initialised = read("gitflow.branch.develop").is_some();

        let fallback = FlowConfig::default();
        let config = FlowConfig {
            main: read("gitflow.branch.master").unwrap_or(fallback.main),
            develop: read("gitflow.branch.develop").unwrap_or(fallback.develop),
            feature: read("gitflow.prefix.feature").unwrap_or(fallback.feature),
            release: read("gitflow.prefix.release").unwrap_or(fallback.release),
            hotfix: read("gitflow.prefix.hotfix").unwrap_or(fallback.hotfix),
        };

        let head = self
            .run_git_reading(&["rev-parse", "--abbrev-ref", "HEAD"])?
            .stdout
            .trim()
            .to_owned();
        let listed = self.run_git_reading(&["branch", "--list", "--format=%(refname:short)"])?;

        let mut branches = Vec::new();
        for line in listed
            .stdout
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            for kind in [FlowKind::Feature, FlowKind::Release, FlowKind::Hotfix] {
                let prefix = config.prefix(kind);
                if prefix.is_empty() {
                    continue;
                }
                if let Some(name) = line.strip_prefix(prefix) {
                    branches.push(FlowBranch {
                        kind,
                        name: name.to_owned(),
                        full: line.to_owned(),
                        is_head: line == head,
                    });
                    break;
                }
            }
        }

        Ok(FlowStatus {
            initialised,
            config,
            branches,
        })
    }

    pub fn flow_init(&self, config: &FlowConfig) -> Result<()> {
        for (key, get) in KEYS {
            self.run_git(&["config", key, get(config)])?;
        }

        if !self.branch_exists(&config.develop) {
            let base = if self.branch_exists(&config.main) {
                config.main.clone()
            } else {
                "HEAD".to_owned()
            };
            self.run_git(&["branch", &config.develop, &base])?;
        }
        Ok(())
    }

    /// The new branch, checked out. Returns its full name.
    pub fn flow_start(&self, kind: FlowKind, name: &str) -> Result<String> {
        let status = self.require_flow()?;
        let name = name.trim();
        if name.is_empty() {
            return Err(GitError::InvalidState("a branch needs a name".to_owned()));
        }

        let full = format!("{}{name}", status.config.prefix(kind));
        if self.branch_exists(&full) {
            return Err(GitError::InvalidState(format!("{full} already exists")));
        }

        let base = status.config.base(kind);
        if !self.branch_exists(base) {
            return Err(GitError::InvalidState(format!("{base} does not exist")));
        }

        self.run_git(&["switch", "--create", &full, base])?;
        Ok(full)
    }

    /// Folds the branch back where it belongs and deletes it. A conflict stops the whole
    /// thing: the branch stays, so the user can settle the merge and finish it again.
    pub fn flow_finish(&self, kind: FlowKind, name: &str, tag: Option<&str>) -> Result<()> {
        let status = self.require_flow()?;
        let full = format!("{}{}", status.config.prefix(kind), name.trim());
        if !self.branch_exists(&full) {
            return Err(GitError::InvalidState(format!("{full} does not exist")));
        }

        let config = &status.config;
        if kind == FlowKind::Feature {
            self.merge_into(&config.develop, &full)?;
        } else {
            // A release and a hotfix both ship: they land on the main branch, get their
            // tag there, and come back to develop so the next work has them.
            self.merge_into(&config.main, &full)?;
            if let Some(label) = tag {
                self.run_git(&["tag", "--annotate", "--message", label, label])?;
            }
            self.merge_into(&config.develop, &config.main)?;
        }

        self.run_git(&["branch", "--delete", "--force", &full])?;
        self.run_git(&["switch", &config.develop]).map(drop)
    }

    fn require_flow(&self) -> Result<FlowStatus> {
        let status = self.flow_status()?;
        if !status.initialised {
            return Err(GitError::InvalidState(
                "Git-Flow is not set up in this repository".to_owned(),
            ));
        }
        Ok(status)
    }

    /// `--no-ff`, or a finished branch leaves no trace of having been one.
    fn merge_into(&self, target: &str, source: &str) -> Result<()> {
        self.run_git(&["switch", target])?;
        self.run_git(&["merge", "--no-ff", "--no-edit", source])
            .map(drop)
    }

    fn branch_exists(&self, name: &str) -> bool {
        self.run_git_reading(&[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{name}"),
        ])
        .is_ok()
    }
}
