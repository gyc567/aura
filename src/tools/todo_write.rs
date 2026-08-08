//! `todo_write` 工具：模型管理任务 TODO 列表。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.3.3 与 §5.5。
//!
//! - 模型创建或更新 TODO 列表；每次写入完整列表，不（增量）。
//! - 工具结果携带 `aura.todo.v1` 版本号，便于未来 schema 演化。
//! - 状态/优先级枚举的字符串与 JSON Schema 一致（`snake_case`）。
//!
//! 工具不需要 capability——这是 Agent 内部状态机的一部分。

use std::sync::Mutex;

use crate::error::AgentError;
use crate::reminders::TodoItem;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};

/// `todo_write` 工具。
///
/// 持有当前 TODO 列表；每次 `execute` 都用新列表整体替换。
pub struct TodoWriteTool {
    state: Mutex<Vec<TodoItem>>,
}

impl TodoWriteTool {
    /// 构造空 TODO 工具。
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(Vec::new()),
        }
    }

    /// 读取当前 TODO 列表（克隆）。
    #[must_use]
    pub fn current(&self) -> Vec<TodoItem> {
        self.state.lock().unwrap().clone()
    }
}

impl Default for TodoWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TodoWriteTool {
    fn name(&self) -> &'static str {
        "todo_write"
    }

    fn description(&self) -> &'static str {
        "Create or update the TODO list. Always write the complete list, not a delta."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": "string", "description": "Stable identifier for this item."},
                                "content": {"type": "string", "description": "What needs to be done."},
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"],
                                    "description": "Current state of this item."
                                },
                                "priority": {
                                    "type": "string",
                                    "enum": ["low", "medium", "high"],
                                    "description": "Relative priority."
                                }
                            },
                            "required": ["id", "content", "status", "priority"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["todos"],
                "additionalProperties": false
            }),
        }
    }

    fn execute(&self, input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        let value = input.arguments.as_value().clone();
        let arr = value
            .get("todos")
            .ok_or_else(|| AgentError::InvalidArguments("missing `todos` field".into()))?
            .clone();
        let todos: Vec<TodoItem> = serde_json::from_value(arr)
            .map_err(|e| AgentError::InvalidArguments(format!("invalid todos: {e}")))?;
        let len = todos.len();
        *self.state.lock().unwrap() = todos;
        Ok(ToolOutput::ok(format!("aura.todo.v1: {len} items written")))
    }
}
