//! 隔离的临时工作空间（Workspace）。

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use crate::bench::spec::SetupAction;

pub struct Workspace {
    root: PathBuf,
    _temp: Option<TempDir>,
}

impl Workspace {
    pub fn new() -> Result<Self, WorkspaceError> {
        let temp = tempfile::Builder::new()
            .prefix("aura-bench-")
            .tempdir()
            .map_err(|e| WorkspaceError::CreateTempDir(e.to_string()))?;
        let root = temp.path().to_path_buf();
        Ok(Self {
            root,
            _temp: Some(temp),
        })
    }

    #[allow(dead_code)]
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, WorkspaceError> {
        let root = path.into();
        if !root.is_dir() {
            std::fs::create_dir_all(&root)
                .map_err(|e| WorkspaceError::CreateTempDir(e.to_string()))?;
        }
        Ok(Self { root, _temp: None })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn setup(&self, actions: &[SetupAction]) -> Result<(), WorkspaceError> {
        for action in actions {
            self.run_action(action)?;
        }
        Ok(())
    }

    fn run_action(&self, action: &SetupAction) -> Result<(), WorkspaceError> {
        match action {
            SetupAction::Write { path, content } => {
                let full = self.root.join(path);
                if let Some(parent) = full.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| WorkspaceError::Setup(e.to_string()))?;
                }
                std::fs::write(&full, content).map_err(|e| WorkspaceError::Setup(e.to_string()))?;
            }
            SetupAction::Mkdir { path } => {
                let full = self.root.join(path);
                std::fs::create_dir_all(&full).map_err(|e| WorkspaceError::Setup(e.to_string()))?;
            }
            SetupAction::Copy { from, to } => {
                let src = self.substitute(from);
                let dst = self.root.join(to);
                if let Some(parent) = dst.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| WorkspaceError::Setup(e.to_string()))?;
                }
                let src_path = PathBuf::from(&src);
                std::fs::copy(&src_path, &dst).map_err(|e| {
                    WorkspaceError::Setup(format!("copy {} to {}: {e}", src, dst.display()))
                })?;
            }
            SetupAction::Clone { repo, depth } => {
                let repo_url = self.substitute(repo);
                let repo_name = std::path::Path::new(&repo_url)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("repo");
                let dest = self.root.join(repo_name);
                let output = Command::new("git")
                    .args(["clone", "--depth", &depth.to_string(), &repo_url])
                    .arg(dest.to_str().unwrap_or("."))
                    .output()
                    .map_err(|e| WorkspaceError::Setup(format!("git clone failed: {e}")))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(WorkspaceError::Setup(format!("git clone failed: {stderr}")));
                }
            }
        }
        Ok(())
    }

    fn substitute(&self, input: &str) -> String {
        input
            .replace("${AURA_WORKSPACE}", self.root.to_str().unwrap_or("."))
            .replace("${AURA_TEMP_DIR}", self.root.to_str().unwrap_or("."))
    }

    pub fn resolve_vars(&self, input: &str) -> String {
        self.substitute(input)
    }

    pub fn resolve(&self, rel: &str) -> PathBuf {
        let resolved = self.substitute(rel);
        if std::path::Path::new(&resolved).is_absolute() {
            PathBuf::from(resolved)
        } else {
            self.root.join(resolved)
        }
    }
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace")
            .field("root", &self.root)
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("failed to create temp dir: {0}")]
    CreateTempDir(String),
    #[error("setup action failed: {0}")]
    Setup(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_workspace() {
        let ws = Workspace::new().unwrap();
        assert!(ws.root().exists());
        assert!(ws.root().to_string_lossy().contains("aura-bench-"));
    }

    #[test]
    fn setup_write_and_read() {
        let ws = Workspace::new().unwrap();
        ws.setup(&[
            SetupAction::Mkdir {
                path: "src".to_string(),
            },
            SetupAction::Write {
                path: "src/main.rs".to_string(),
                content: "fn main() {}".to_string(),
            },
        ])
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(ws.root().join("src/main.rs")).unwrap(),
            "fn main() {}"
        );
    }

    #[test]
    fn resolve_vars() {
        let ws = Workspace::new().unwrap();
        let resolved = ws.resolve_vars("${AURA_WORKSPACE}/src");
        assert!(resolved.starts_with(ws.root().to_string_lossy().as_ref()));
        assert!(resolved.ends_with("/src"));
    }
}
