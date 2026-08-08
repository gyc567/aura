//! `run_command` 工具：执行命令。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.3.2。
//!
//! - 始终使用 argv 模式（不使用 shell），防止注入。
//! - 超时默认 120 秒。
//! - stderr 与 stdout 合并输出，超 64KB 截断。

use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::AgentError;
use crate::policy::Capability;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};

/// `run_command` 工具。
pub struct RunCommandTool {
    /// 超时（秒）。
    timeout_secs: u64,
    /// 输出截断字节数。
    max_output_bytes: usize,
}

impl RunCommandTool {
    /// 构造器。
    #[must_use]
    pub fn new() -> Self {
        Self {
            timeout_secs: 120,
            max_output_bytes: 65_536,
        }
    }

    /// 设置超时。
    #[must_use]
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

impl Default for RunCommandTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for RunCommandTool {
    fn name(&self) -> &'static str {
        "run_command"
    }

    fn description(&self) -> &'static str {
        "Execute a shell command. Always uses argv (no shell interpretation). Captures stdout+stderr, returns exit code."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to execute (first token is the program)."
                    },
                    "args": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Command arguments."
                    },
                    "working_directory": {
                        "type": "string",
                        "description": "Working directory for the command (must be within workspace)."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    fn required_capabilities(&self) -> &'static [Capability] {
        &[Capability::Exec]
    }

    fn needs_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, input: ToolInput, ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        let value = input.arguments.as_value();
        let command = value
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::InvalidArguments("missing `command` field".into()))?;

        let args: Vec<String> = value
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let argv: Vec<String> = std::iter::once(command.to_string())
            .chain(args.iter().cloned())
            .collect();

        // 预检（regex 高危模式检测）
        let precheck = crate::precheck::analyze(&argv)?;
        if precheck.tier == crate::precheck::RiskTier::High {
            return Err(AgentError::CommandPolicy(format!(
                "high-risk command blocked: categories={:?}",
                precheck.categories
            )));
        }

        // 工作目录
        let work_dir = if let Some(wd) = value.get("working_directory").and_then(|v| v.as_str()) {
            let rel = Path::new(wd);
            let abs = if rel.is_absolute() {
                rel.to_path_buf()
            } else {
                ctx.workspace.join(rel)
            };
            let abs = abs.canonicalize().unwrap_or_else(|_| abs);
            if !abs.starts_with(&ctx.workspace) {
                return Err(AgentError::PathPolicy(format!(
                    "working_directory {} escapes workspace",
                    abs.display()
                )));
            }
            abs
        } else {
            ctx.workspace.clone()
        };

        let output = Command::new(command)
            .args(&args)
            .current_dir(&work_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| AgentError::Context(format!("failed to spawn {}: {e}", command)))?;

        let stdout = output.stdout;
        let stderr = output.stderr;
        let exit_code = output.status.code().unwrap_or(-1);

        let mut combined = stdout;
        combined.extend_from_slice(&stderr);
        let truncated = if combined.len() > self.max_output_bytes {
            combined.truncate(self.max_output_bytes);
            combined.extend_from_slice(b"\n... (output truncated)");
            true
        } else {
            false
        };

        let stdout_str = String::from_utf8_lossy(&combined);
        let summary = if truncated {
            format!(
                "exit={}\n{}",
                exit_code,
                stdout_str
            )
        } else {
            format!(
                "exit={}\n{}",
                exit_code,
                stdout_str
            )
        };

        if exit_code == 0 {
            Ok(ToolOutput::ok(summary))
        } else {
            Ok(ToolOutput::err(summary))
        }
    }
}
