//! 任务执行器（TaskRunner）：在 Workspace 中运行 agent 并验证结果。

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::bench::spec::{TaskSpec, VerifySpec};
use crate::bench::workspace::Workspace;

/// 任务执行结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// 任务 ID。
    pub task_id: String,
    /// 任务名称。
    pub task_name: String,
    /// 难度。
    pub difficulty: String,
    /// 分类。
    pub category: String,
    /// 执行状态。
    pub status: TaskStatus,
    /// 验证退出码。
    pub verify_exit_code: Option<i32>,
    /// Agent wall time (秒)。
    pub agent_wall_time_s: f64,
    /// Agent 使用的 turns。
    pub agent_turns: u32,
    /// 错误信息。
    pub error: Option<String>,
    /// 失败时 workspace 快照路径。
    pub workspace_snapshot: Option<String>,
}

/// 任务执行状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Passed,
    Failed,
    Timeout,
    Error,
}

/// 单个任务的执行器。
pub struct TaskRunner {
    agent_cmd: String,
    default_timeout_s: u64,
}

impl TaskRunner {
    pub fn new() -> Self {
        Self {
            agent_cmd: "cargo run --bin aura".to_string(),
            default_timeout_s: 300,
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn agent_cmd(mut self, cmd: &str) -> Self {
        self.agent_cmd = cmd.to_string();
        self
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn default_timeout(mut self, seconds: u64) -> Self {
        self.default_timeout_s = seconds;
        self
    }

    /// 运行单个任务。
    #[must_use]
    pub fn run(&self, spec: &TaskSpec, workspace: &Workspace) -> TaskResult {
        let start = Instant::now();

        if let Err(e) = workspace.setup(&spec.setup) {
            return TaskResult {
                task_id: spec.id.clone(),
                task_name: spec.name.clone(),
                difficulty: spec.difficulty.to_string(),
                category: spec.category.to_string(),
                status: TaskStatus::Error,
                verify_exit_code: None,
                agent_wall_time_s: start.elapsed().as_secs_f64(),
                agent_turns: 0,
                error: Some(format!("setup failed: {e}")),
                workspace_snapshot: None,
            };
        }

        let instruction = spec.instruction.clone();
        let agent_result = self.run_agent(workspace.root(), &instruction);
        let agent_wall_time = start.elapsed().as_secs_f64();
        let agent_turns = Self::extract_turns(&agent_result.stdout);

        let verify_exit_code = self.verify(&spec.verify, workspace);

        let status = if agent_result.timed_out {
            TaskStatus::Timeout
        } else if agent_result.error.is_some() {
            TaskStatus::Error
        } else if verify_exit_code != 0 {
            TaskStatus::Failed
        } else {
            TaskStatus::Passed
        };

        let error = agent_result.error.or_else(|| {
            if verify_exit_code != 0 {
                Some(format!("verify failed with exit code {verify_exit_code}"))
            } else {
                None
            }
        });

        TaskResult {
            task_id: spec.id.clone(),
            task_name: spec.name.clone(),
            difficulty: spec.difficulty.to_string(),
            category: spec.category.to_string(),
            status,
            verify_exit_code: Some(verify_exit_code),
            agent_wall_time_s: agent_wall_time,
            agent_turns,
            error,
            workspace_snapshot: None,
        }
    }

    fn run_agent(&self, workspace_root: &Path, instruction: &str) -> AgentOutput {
        let escaped = instruction.replace('\'', "'\\''");
        let cmd = format!(
            "cd '{}' && {} --json --workspace '{}' -- '{}' 2>&1",
            workspace_root.display(),
            self.agent_cmd,
            workspace_root.display(),
            escaped,
        );

        match Command::new("sh").arg("-c").arg(&cmd).output() {
            Ok(out) => AgentOutput {
                stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                timed_out: false,
                error: None,
            },
            Err(e) => AgentOutput {
                stdout: String::new(),
                stderr: String::new(),
                timed_out: false,
                error: Some(e.to_string()),
            },
        }
    }

    fn verify(&self, spec: &VerifySpec, workspace: &Workspace) -> i32 {
        match spec {
            VerifySpec::Command {
                command,
                cwd,
                timeout_seconds: _,
            } => {
                let cwd_path = workspace.resolve(cwd);
                let shell_cmd = workspace.resolve_vars(command);
                Self::run_shell(&shell_cmd, &cwd_path)
            }
            VerifySpec::FileExists { path } => {
                let resolved = workspace.resolve(path);
                i32::from(!resolved.is_file())
            }
            VerifySpec::CargoTest { timeout_seconds: _ } => {
                Self::run_shell("cargo test --quiet 2>&1", workspace.root())
            }
            VerifySpec::CargoFmt => Self::run_shell("cargo fmt --check 2>&1", workspace.root()),
            VerifySpec::GitDiff { pattern: _ } => 0,
        }
    }

    fn run_shell(cmd: &str, cwd: &Path) -> i32 {
        match Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(cwd)
            .output()
        {
            Ok(out) => out.status.code().unwrap_or(-1),
            Err(_) => -1,
        }
    }

    fn extract_turns(stdout: &str) -> u32 {
        if let Ok(report) = serde_json::from_str::<serde_json::Value>(stdout) {
            if let Some(turns) = report.get("used_turns").and_then(serde_json::Value::as_u64) {
                return u32::try_from(turns).unwrap_or(0);
            }
        }
        0
    }
}

impl Default for TaskRunner {
    fn default() -> Self {
        Self::new()
    }
}

struct AgentOutput {
    stdout: String,
    stderr: String,
    timed_out: bool,
    error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_runner() {
        let runner = TaskRunner::new();
        assert_eq!(runner.agent_cmd, "cargo run --bin aura");
        assert_eq!(runner.default_timeout_s, 300);
    }

    #[test]
    fn runner_with_custom_agent() {
        let runner = TaskRunner::new()
            .agent_cmd("claude-code")
            .default_timeout(600);
        assert_eq!(runner.agent_cmd, "claude-code");
        assert_eq!(runner.default_timeout_s, 600);
    }
}
