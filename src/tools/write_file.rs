//! `write_file` 工具：写入工作区内文件。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.3.3。
//!
//! - 路径必须在 workspace 内。
//! - 拒绝敏感路径。
//! - 必须通过 Policy capability gate（由调用方在执行前检查）。
//! - v1 Non-interactive：不打印 unified diff；`--yes` 跳过 confirmation。

use std::path::Path;

use crate::error::AgentError;
use crate::policy::Capability;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};

/// `write_file` 工具。
pub struct WriteFileTool {
    /// 最大文件大小（字节）。
    max_bytes: usize,
}

impl WriteFileTool {
    /// 构造器。
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_bytes: 10_000_000,
        }
    }

    /// 设置字节上限。
    #[must_use]
    pub fn with_max_bytes(mut self, bytes: usize) -> Self {
        self.max_bytes = bytes;
        self
    }

    fn resolve_and_check(path: &str, workspace: &Path) -> Result<std::path::PathBuf, AgentError> {
        let rel = Path::new(path);
        let abs = if rel.is_absolute() {
            rel.to_path_buf()
        } else {
            workspace.join(rel)
        };
        let abs = abs.canonicalize().unwrap_or_else(|_| abs.clone());
        if !abs.starts_with(workspace) {
            return Err(AgentError::PathPolicy(format!(
                "path {} escapes workspace {}",
                path,
                workspace.display()
            )));
        }
        Ok(abs)
    }
}

impl Default for WriteFileTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Write or overwrite a file in the workspace. Use this to create new files or update existing ones. The file path must be within the workspace."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (relative to workspace or absolute)."
                    },
                    "content": {
                        "type": "string",
                        "description": "Full file content to write."
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    fn required_capabilities(&self) -> &'static [Capability] {
        &[Capability::FsWrite]
    }

    fn needs_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, input: ToolInput, ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        let value = input.arguments.as_value();
        let path = value
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::InvalidArguments("missing `path` field".into()))?;
        let content = value
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::InvalidArguments("missing `content` field".into()))?;

        if content.len() > self.max_bytes {
            return Err(AgentError::Context(format!(
                "content exceeds {} bytes",
                self.max_bytes
            )));
        }

        let abs = Self::resolve_and_check(path, &ctx.workspace)?;

        // 拒绝敏感路径
        if crate::context::is_sensitive(&abs) {
            return Err(AgentError::PathPolicy(format!(
                "sensitive path refused: {}",
                abs.display()
            )));
        }

        // 确保父目录存在
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AgentError::Context(format!("mkdir {} failed: {e}", parent.display()))
            })?;
        }

        std::fs::write(&abs, content)
            .map_err(|e| AgentError::Context(format!("write {} failed: {e}", abs.display())))?;

        Ok(ToolOutput::ok(format!(
            "Written {} bytes to {}",
            content.len(),
            abs.display()
        )))
    }
}
