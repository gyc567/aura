//! `find_files` 工具：递归搜索文件名。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.3.3。
//!
//! - 按文件名模式递归搜索。
//! - 返回匹配文件的相对路径。

use std::path::Path;

use crate::error::AgentError;
use crate::policy::Capability;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};

/// `find_files` 工具。
pub struct FindFilesTool {
    /// 最大结果数。
    max_results: usize,
}

impl FindFilesTool {
    /// 构造器。
    #[must_use]
    pub fn new() -> Self {
        Self { max_results: 100 }
    }
}

impl Default for FindFilesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for FindFilesTool {
    fn name(&self) -> &'static str {
        "find_files"
    }

    fn description(&self) -> &'static str {
        "Find files by name pattern (substring match). Returns paths relative to workspace. Maximum 100 results."
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
                        "description": "Root directory to search from (relative to workspace or absolute)."
                    },
                    "pattern": {
                        "type": "string",
                        "description": "File name pattern (substring match, case-insensitive)."
                    }
                },
                "required": ["path", "pattern"]
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
        let pattern = value
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::InvalidArguments("missing `pattern` field".into()))?;

        let rel = Path::new(path);
        let abs = if rel.is_absolute() {
            rel.to_path_buf()
        } else {
            ctx.workspace.join(rel)
        };
        let abs = abs.canonicalize().unwrap_or_else(|_| abs.clone());
        if !abs.starts_with(&ctx.workspace) {
            return Err(AgentError::PathPolicy(format!(
                "path {} escapes workspace",
                abs.display()
            )));
        }

        let pattern_lower = pattern.to_lowercase();
        let mut found: Vec<String> = Vec::new();
        let mut count = 0;

        self.find_dir(&abs, &ctx.workspace, &pattern_lower, &mut found, &mut count)?;

        if found.is_empty() {
            return Ok(ToolOutput::ok(format!(
                "No files matching '{}' under {}",
                pattern,
                abs.display()
            )));
        }

        Ok(ToolOutput::ok(format!(
            "Found {} files:\n\n{}",
            found.len(),
            found.join("\n")
        )))
    }
}

impl FindFilesTool {
    fn find_dir(
        &self,
        dir: &Path,
        workspace: &Path,
        pattern: &str,
        found: &mut Vec<String>,
        count: &mut usize,
    ) -> Result<(), AgentError> {
        if *count >= self.max_results {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir)
            .map_err(|e| AgentError::Context(format!("read_dir {:?} failed: {e}", dir)))?;

        for entry in entries.filter_map(|e| e.ok()) {
            if *count >= self.max_results {
                break;
            }
            let path = entry.path();

            if path
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            if path.is_dir() {
                self.find_dir(&path, workspace, pattern, found, count)?;
            } else if path.is_file() {
                if crate::context::is_sensitive(&path) {
                    continue;
                }
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if name.contains(pattern) {
                    *count += 1;
                    let rel = path.strip_prefix(workspace)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| path.display().to_string());
                    found.push(rel);
                }
            }
        }
        Ok(())
    }
}
