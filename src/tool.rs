//! 工具 trait 与上下文。
//!
//! Phase 1 只定义接口；具体实现在 Phase 2 加入。工具实现不互相调用，
//! 所有 IO 通过 [`ToolContext`] 注入的参数完成。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::ToolArgument;
use crate::error::AgentError;

/// 工具执行时可访问的只读上下文。
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// 工作区根目录。
    pub workspace: PathBuf,
    /// 工具调用 id。
    pub call_id: String,
}

impl ToolContext {
    /// 创建工具上下文。
    #[must_use]
    pub fn new(workspace: PathBuf, call_id: impl Into<String>) -> Self {
        Self {
            workspace,
            call_id: call_id.into(),
        }
    }
}

/// 工具输入。
#[derive(Debug, Clone)]
pub struct ToolInput {
    /// 调用参数。
    pub arguments: ToolArgument,
}

impl ToolInput {
    /// 创建工具输入。
    #[must_use]
    pub fn new(arguments: ToolArgument) -> Self {
        Self { arguments }
    }
}

/// 工具输出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    /// 输出文本。
    pub content: String,
    /// 是否成功。
    pub success: bool,
}

impl ToolOutput {
    /// 创建成功输出。
    #[must_use]
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            success: true,
        }
    }

    /// 创建失败输出。
    #[must_use]
    pub fn err(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            success: false,
        }
    }
}

/// 工具的元数据描述，供模型与 CLI 展示。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolSchema {
    /// 工具名。
    pub name: String,
    /// 工具说明。
    pub description: String,
    /// JSON Schema 格式的参数定义。Phase 2 起使用；Phase 1 工具可填 `{}`。
    pub parameters: serde_json::Value,
}

impl ToolSchema {
    /// 构造工具描述。参数默认空对象 `{}`，调用方可在构建后赋值 `parameters`。
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: serde_json::Value::Object(serde_json::Map::new()),
        }
    }
}

/// 工具 trait。
pub trait Tool: Send + Sync {
    /// 工具名，与 `Decision::Call` 中的 `name` 字段一致。
    fn name(&self) -> &'static str;

    /// 工具说明。
    fn description(&self) -> &'static str;

    /// 工具元数据。
    fn schema(&self) -> ToolSchema {
        ToolSchema::new(self.name(), self.description())
    }

    /// 工具所需 capability。默认空数组——纯内部工具（`todo_write` 等）使用默认值。
    fn required_capabilities(&self) -> &'static [super::Capability] {
        &[]
    }

    /// 是否需要 CLI `--yes` 或人工确认后才执行。默认 false（只读工具）。
    fn needs_confirmation(&self) -> bool {
        false
    }

    /// 执行工具。
    ///
    /// # Errors
    ///
    /// - [`AgentError::PathPolicy`] / [`AgentError::CommandPolicy`]：违反策略。
    /// - [`AgentError::ToolFailed`]：具体执行失败。
    fn execute(&self, input: ToolInput, ctx: &ToolContext) -> Result<ToolOutput, AgentError>;
}
