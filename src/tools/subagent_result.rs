//! `subagent_result` 工具：父代理收集子代理状态/结果 (Architecture §4.2)。
//!
//! 输入: `{ child_id }` → 返回 `{ child_id, name, status, result }`。
//! - status: `running` | `completed` | `failed`
//! - result: 子代理完成后的结果摘要;未完成时为 `null`
//!
//! 非同步等待:子代理后台运行,父代理在后续轮次轮询本工具。

use std::sync::Arc;

use crate::children::ChildRegistry;
use crate::domain::ChildId;
use crate::error::AgentError;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};
use serde::Deserialize;

/// `subagent_result` 工具参数。
#[derive(Debug, Deserialize)]
struct SubagentResultInput {
    /// 子代理 ID（由 `subagent` 工具返回）。
    child_id: String,
}

/// `subagent_result` 工具。
pub struct SubagentResultTool {
    registry: Arc<ChildRegistry>,
}

impl SubagentResultTool {
    /// 创建 `subagent_result` 工具。
    #[must_use]
    pub fn new(registry: Arc<ChildRegistry>) -> Self {
        Self { registry }
    }
}

impl Tool for SubagentResultTool {
    fn name(&self) -> &'static str {
        "subagent_result"
    }

    fn description(&self) -> &'static str {
        "Fetch the status and result of a previously spawned sub-agent by child_id. Returns { child_id, name, status, result }. status is one of running | completed | failed; result is null while the child is still running — call again later."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "child_id": {
                        "type": "string",
                        "description": "Child agent ID returned by the subagent tool."
                    }
                },
                "required": ["child_id"],
                "additionalProperties": false
            }),
        }
    }

    fn execute(&self, input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        let args: SubagentResultInput = serde_json::from_value(input.arguments.into_value())
            .map_err(|e| AgentError::InvalidArguments(format!("subagent_result args: {e}")))?;

        if args.child_id.trim().is_empty() {
            return Err(AgentError::InvalidArguments(
                "child_id must not be empty".into(),
            ));
        }

        let child_id = ChildId(args.child_id);
        let handle = self.registry.get(&child_id).ok_or_else(|| {
            AgentError::InvalidRequest(format!("child agent not found: {child_id}"))
        })?;

        let status = match handle.status {
            crate::domain::ChildStatus::Running => "running",
            crate::domain::ChildStatus::Completed => "completed",
            crate::domain::ChildStatus::Failed => "failed",
        };

        let payload = serde_json::json!({
            "child_id": child_id,
            "name": handle.name,
            "status": status,
            "result": handle.result,
        });
        Ok(ToolOutput::ok(payload.to_string()))
    }
}
