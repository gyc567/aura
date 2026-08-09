//! `read_file` 工具：读取工作区内文件。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.3.3。
//!
//! - 路径必须在 workspace 内（使用 `ToolContext::workspace`）。
//! - 拒绝敏感路径（`.env`、`.pem`、`.ssh` 等）。
//! - 字节上限：1MB。
//! - 拒绝二进制文件（检测 `\0` 字节）。

use std::path::Path;

use crate::error::AgentError;
use crate::policy::Capability;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};

/// `read_file` 工具。
pub struct ReadFileTool {
    /// 字节上限。
    max_bytes: usize,
}

impl ReadFileTool {
    /// 构造器。
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_bytes: 1_000_000,
        }
    }

    /// 设置字节上限。
    #[must_use]
    pub fn with_max_bytes(mut self, bytes: usize) -> Self {
        self.max_bytes = bytes;
        self
    }

    fn resolve_and_check(path: &str, workspace: &Path) -> Result<std::path::PathBuf, AgentError> {
        crate::paths::resolve_in_workspace(Path::new(path), workspace)
    }
}

impl Default for ReadFileTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a file. The file must be within the workspace. Returns up to 1MB of text."
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
                        "description": "Relative or absolute path to the file."
                    }
                },
                "required": ["path"]
            }),
        }
    }

    fn required_capabilities(&self) -> &'static [Capability] {
        &[Capability::FsRead]
    }

    fn execute(&self, input: ToolInput, ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        let value = input.arguments.as_value();
        let path = value
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::InvalidArguments("missing `path` field".into()))?;

        let abs = Self::resolve_and_check(path, &ctx.workspace)?;

        // 拒绝敏感路径
        if crate::context::is_sensitive(&abs) {
            return Err(AgentError::PathPolicy(format!(
                "sensitive path refused: {}",
                abs.display()
            )));
        }

        let metadata = std::fs::metadata(&abs)
            .map_err(|e| AgentError::Context(format!("stat {} failed: {e}", abs.display())))?;

        if metadata.len() > self.max_bytes as u64 {
            return Err(AgentError::Context(format!(
                "file {} exceeds {} bytes",
                abs.display(),
                self.max_bytes
            )));
        }

        let bytes = std::fs::read(&abs)
            .map_err(|e| AgentError::Context(format!("read {} failed: {e}", abs.display())))?;

        // 检测二进制
        if bytes.contains(&0) {
            return Err(AgentError::Context(format!(
                "binary file {} not readable as text",
                abs.display()
            )));
        }

        let content = String::from_utf8(bytes)
            .map_err(|_| AgentError::Context(format!("{} is not valid UTF-8", abs.display())))?;

        Ok(ToolOutput::ok(format!(
            "File: {}\n\n{}",
            abs.display(),
            content
        )))
    }
}
