//! `agent_message` 工具：向背景子代理发送定向消息 (Architecture §4.2)。
//!
//! parent → child 定向消息(邮箱队列)；child 通过同一工具回复 parent。

use std::sync::Arc;

use crate::children::ChildRegistry;
use crate::domain::{AgentMessage, ChildId};
use crate::error::AgentError;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};
use serde::Deserialize;

/// `agent_message` 工具参数。
#[derive(Debug, Deserialize)]
struct MessageInput {
    to: String,
    content: String,
}

/// `agent_message` 工具。
pub struct AgentMessageTool {
    registry: Arc<ChildRegistry>,
}

impl AgentMessageTool {
    /// 创建 `agent_message` 工具。
    #[must_use]
    pub fn new(registry: Arc<ChildRegistry>) -> Self {
        Self { registry }
    }
}

impl Tool for AgentMessageTool {
    fn name(&self) -> &'static str {
        "agent_message"
    }

    fn description(&self) -> &'static str {
        "Send a message to a running sub-agent by child_id. The message is delivered to the child's inbox and will be processed in its next turn."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "to": { "type": "string", "description": "Child agent ID." },
                    "content": { "type": "string", "description": "Message content." }
                },
                "required": ["to", "content"],
                "additionalProperties": false
            }),
        }
    }

    fn execute(&self, input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        let args: MessageInput = serde_json::from_value(input.arguments.into_value())
            .map_err(|e| AgentError::InvalidArguments(format!("agent_message args: {e}")))?;

        if args.to.trim().is_empty() {
            return Err(AgentError::InvalidArguments(
                "recipient (to) must not be empty".into(),
            ));
        }

        let child_id = ChildId(args.to);
        let msg = AgentMessage {
            to: child_id.clone(),
            from: "parent".to_string(),
            content: args.content,
        };

        if self.registry.send_message(&child_id, msg) {
            Ok(ToolOutput::ok(format!("Message sent to child {child_id}")))
        } else {
            Err(AgentError::InvalidRequest(format!(
                "child agent not found: {child_id}"
            )))
        }
    }
}
