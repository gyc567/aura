//! `grep_files` 工具：递归搜索文件内容。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.3.3。
//!
//! - 递归搜索匹配 pattern 的文件内容。
//! - 返回匹配行（带行号）。
//! - 最大结果行数：100 行。

use std::path::Path;

use crate::error::AgentError;
use crate::policy::Capability;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};

/// `grep_files` 工具。
pub struct GrepFilesTool {
    /// 最大结果行数。
    max_lines: usize,
}

impl GrepFilesTool {
    /// 构造器。
    #[must_use]
    pub fn new() -> Self {
        Self { max_lines: 100 }
    }
}

impl Default for GrepFilesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for GrepFilesTool {
    fn name(&self) -> &'static str {
        "grep_files"
    }

    fn description(&self) -> &'static str {
        "Search for a pattern in files under a directory. Returns matching lines with file:line:content format. Maximum 100 results."
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
                        "description": "Directory to search in (relative to workspace or absolute)."
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Search pattern (substring match)."
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
        let mut results: Vec<String> = Vec::new();
        let mut count = 0;

        self.grep_dir(&abs, &pattern_lower, &mut results, &mut count)?;

        if results.is_empty() {
            return Ok(ToolOutput::ok(format!(
                "No matches for '{}' under {}",
                pattern,
                abs.display()
            )));
        }

        Ok(ToolOutput::ok(format!(
            "Found {} matches:\n\n{}",
            count,
            results.join("\n")
        )))
    }
}

impl GrepFilesTool {
    fn grep_dir(
        &self,
        dir: &Path,
        pattern: &str,
        results: &mut Vec<String>,
        count: &mut usize,
    ) -> Result<(), AgentError> {
        if *count >= self.max_lines {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir)
            .map_err(|e| AgentError::Context(format!("read_dir {:?} failed: {e}", dir)))?;

        for entry in entries.filter_map(|e| e.ok()) {
            if *count >= self.max_lines {
                break;
            }
            let path = entry.path();

            // 跳过隐藏文件/目录
            if path
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            if path.is_dir() {
                self.grep_dir(&path, pattern, results, count)?;
            } else if path.is_file() {
                // 跳过二进制和敏感文件
                if crate::context::is_sensitive(&path) {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if *count >= self.max_lines {
                            break;
                        }
                        if line.to_lowercase().contains(pattern) {
                            *count += 1;
                            results.push(format!(
                                "{}:{}:{}",
                                path.display(),
                                line_num + 1,
                                line
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
