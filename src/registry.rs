//! 工具注册表抽象与内存实现。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.6。
//! - 集中按工具名查找并分发。
//! - 不在工具实现内部做 capability 校验——`Capability` 由 [`aura::policy`] 模块评估。
//! - 未来若需要按 capability 过滤，可派生出 `CapabilityAwareRegistry`。

use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::ToolCall;
use crate::error::AgentError;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};

/// 工具注册表 trait。
///
/// 所有实现必须为 `Send + Sync`，以便在 `tokio` 单线程运行时（Phase 3）共享。
pub trait ToolRegistry: Send + Sync {
    /// 执行一次工具调用。
    ///
    /// # Errors
    ///
    /// - [`AgentError::UnknownTool`]：工具名不在注册表中。
    /// - 工具自身 `execute` 返回的任何错误（透传）。
    fn execute(&self, call: &ToolCall, ctx: &ToolContext) -> Result<ToolOutput, AgentError>;

    /// 列出全部可用工具的 schema，供模型发现工具（发送到 provider 的 `tools` 字段）。
    fn schemas(&self) -> Vec<ToolSchema> {
        Vec::new()
    }
}

/// 内存版注册表。基于 `HashMap<name, Arc<dyn Tool>>` 实现。
pub struct InMemoryRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl InMemoryRegistry {
    /// 用一组工具构造注册表；同名工具后者覆盖前者（v1 行为：先注册先赢）。
    #[must_use]
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        let mut map = HashMap::new();
        for tool in tools {
            map.insert(tool.name().to_string(), tool);
        }
        Self { tools: map }
    }

    /// 构造空注册表。
    #[must_use]
    pub fn empty() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// 是否包含指定工具。
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// 已注册工具数量。
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// 是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// 列出所有工具的 schema。供模型发现工具。
    #[must_use]
    pub fn schemas(&self) -> Vec<ToolSchema> {
        self.tools.values().map(|t| t.schema()).collect()
    }
}

impl ToolRegistry for InMemoryRegistry {
    fn execute(&self, call: &ToolCall, ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        let tool = self
            .tools
            .get(&call.name)
            .ok_or_else(|| AgentError::UnknownTool(call.name.clone()))?;
        let input = ToolInput::new(call.arguments.clone());
        tool.execute(input, ctx)
    }

    fn schemas(&self) -> Vec<ToolSchema> {
        self.schemas()
    }
}
