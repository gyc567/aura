//! 模型网关 trait。
//!
//! 第一版定义接口与请求/响应类型。HTTP / OpenAI-compatible 适配器
//! 由 [`crate::model_http`] 提供；测试通过 `FakeModel` 提供确定性响应。
//!
//! 采用 `impl Future + Send` 形式而非原生 `async fn` in trait：
//! - 明确 future 的 `Send` 边界，使 `ModelGateway: Send + Sync` 与多线程
//!   运行时配合时不出现隐式 non-Send 警告。
//! - 避免未来若需 `dyn ModelGateway` 时被 `async_fn_in_trait` 限制。
//! - 任何实现方都需自行保证 future 真的 `Send`（持有非 `Send` 状态会导致
//!   编译错误，而不是运行时 panic）。

use std::future::Future;
use std::pin::Pin;

use crate::domain::{Decision, Message};
use crate::error::AgentError;

/// 单次模型调用所需的所有信息。
#[derive(Debug, Clone)]
pub struct ModelRequest {
    /// 系统提示词。
    pub system: String,
    /// 对话历史。
    pub messages: Vec<Message>,
    /// 工具描述列表。
    pub tool_schemas: Vec<crate::tool::ToolSchema>,
}

impl ModelRequest {
    /// 创建请求。系统提示词允许为空字符串。
    #[must_use]
    pub fn new(system: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            system: system.into(),
            messages,
            tool_schemas: Vec::new(),
        }
    }

    /// 附带工具描述。
    #[must_use]
    pub fn with_tool_schemas(mut self, schemas: Vec<crate::tool::ToolSchema>) -> Self {
        self.tool_schemas = schemas;
        self
    }
}

/// 模型返回的结构化结果。
#[derive(Debug, Clone)]
pub struct ModelResponse {
    /// 解析后的决策。
    pub decision: Decision,
    /// 模型返回的原始文本，用于日志/审计。
    pub raw: String,
}

/// 流式响应事件。Phase 4+ 升级为真正的 SSE；v1 仅单事件。
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// 增量文本。
    Delta(String),
    /// 完整响应。
    Complete(ModelResponse),
}

/// 流式响应聚合。
#[derive(Debug, Clone)]
pub struct ModelStream {
    events: Vec<StreamEvent>,
}

impl ModelStream {
    /// 从单次响应构造单事件流（v1 默认实现）。
    #[must_use]
    pub fn from_response(resp: ModelResponse) -> Self {
        Self {
            events: vec![StreamEvent::Complete(resp)],
        }
    }

    /// 取出所有事件（消费流）。
    #[must_use]
    pub fn into_events(self) -> Vec<StreamEvent> {
        self.events
    }

    /// 是否空流。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// 流长度（事件数）。
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }
}

/// 模型网关 trait。
///
/// 任何 provider 适配器只需实现该 trait。`async fn` 的 future 通过 `Pin<Box<dyn Future + Send>>`
/// 显化，使 trait 为 dyn-compatible（v0.5 §5.2 设计）。
///
/// Future 必须为 `Send` 以便在多线程运行时中安全共享。
pub trait ModelGateway: Send + Sync {
    /// 执行一次补全（非流式）。
    ///
    /// # Errors
    ///
    /// - [`AgentError::UnparseableDecision`]：模型返回无法解析为合法 `Decision`。
    /// - [`AgentError::Context`]：HTTP / IO / 配置错误。
    fn complete(
        &self,
        request: ModelRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ModelResponse, AgentError>> + Send + '_>>;

    /// 流式补全。默认实现退化为单事件流（v1）。
    ///
    /// # Errors
    ///
    /// 同 [`Self::complete`]。
    fn stream(
        &self,
        request: ModelRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ModelStream, AgentError>> + Send + '_>> {
        Box::pin(async move {
            let resp = self.complete(request).await?;
            Ok(ModelStream::from_response(resp))
        })
    }
}
