//! `list_dir` 工具：列出目录内容。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.3.3。
//!
//! - 返回目录条目的文件名与类型（file/dir）。
//! - 不递归。

use std::path::Path;

use crate::error::AgentError;
use crate::policy::Capability;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};

/// `list_dir` 工具。
#[derive(Default)]
pub struct ListDirTool;

impl ListDirTool {
    /// 构造器。
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Tool for ListDirTool {
    fn name(&self) -> &'static str {
        "list_dir"
    }

    fn description(&self) -> &'static str {
        "List the contents of a directory. Returns file and directory names (non-recursive)."
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
                        "description": "Directory path (relative to workspace or absolute)."
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

        let abs = crate::paths::resolve_in_workspace(Path::new(path), &ctx.workspace)?;

        if !abs.is_dir() {
            return Err(AgentError::Context(format!(
                "{} is not a directory",
                abs.display()
            )));
        }

        let dir = std::fs::read_dir(&abs)
            .map_err(|e| AgentError::Context(format!("read_dir {} failed: {e}", abs.display())))?;

        let mut names: Vec<String> = dir
            .filter_map(std::result::Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    return None; // 跳过隐藏文件
                }
                let kind = if e.path().is_dir() { "[dir]" } else { "[file]" };
                Some(format!("{name:<40} {kind}"))
            })
            .collect();
        names.sort();

        if names.is_empty() {
            Ok(ToolOutput::ok(format!(
                "(empty directory: {})",
                abs.display()
            )))
        } else {
            Ok(ToolOutput::ok(format!(
                "{}\n{}",
                abs.display(),
                names.join("\n")
            )))
        }
    }
}
