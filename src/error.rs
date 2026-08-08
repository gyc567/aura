//! 智能体错误类型。
//!
//! 所有错误都实现 `std::error::Error` 并保留 source。错误分类与恢复策略
//! 见 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §7。

use thiserror::Error;

/// 智能体所有可恢复与不可恢复错误的统一类型。
///
/// 领域层只使用本类型，避免具体 IO / 模型 SDK 错误渗透到核心。
#[derive(Debug, Error)]
pub enum AgentError {
    /// 任务请求不合法（空指令、非正整数上限、workspace 路径不存在等）。
    #[error("invalid task request: {0}")]
    InvalidRequest(String),

    /// 状态机非法转移。
    #[error("invalid state transition: {0}")]
    InvalidTransition(String),

    /// 预算耗尽（轮次、字节或时间）。
    #[error("budget exhausted: {0}")]
    BudgetExhausted(String),

    /// 模型输出无法解析为合法 `Decision`。
    #[error("model output unparseable: {0}")]
    UnparseableDecision(String),

    /// 调用了未注册或被禁用的工具。
    #[error("unknown tool: {0}")]
    UnknownTool(String),

    /// 工具执行失败。
    #[error("tool execution failed: {0}")]
    ToolFailed(String),

    /// 验证阶段（测试、lint 等）失败。
    #[error("verification failed: {0}")]
    VerificationFailed(String),

    /// 路径越界或敏感路径。
    #[error("path policy violation: {0}")]
    PathPolicy(String),

    /// 命令策略拒绝执行。
    #[error("command policy violation: {0}")]
    CommandPolicy(String),

    /// 上下文收集阶段错误。
    #[error("context error: {0}")]
    Context(String),

    /// 工具参数不符合 schema 或解析失败。
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),

    /// 需要人工/CLI `--yes` 确认后才能执行。
    #[error("confirmation required: {0}")]
    NeedsConfirmation(String),
}

impl AgentError {
    /// 返回该错误是否值得自动重试。
    ///
    /// 当前仅在调用方提供重试语义时使用；如策略在后续版本收紧，
    /// 应对每条错误给出明确判断而不是默认 `true`。
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        // 当前实现：仅 IO 相关瞬态错误可重试。领域错误一律不可重试。
        matches!(self, Self::Context(_))
    }

    /// 返回该错误对应的 CLI 退出码。
    ///
    /// 见 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §7：
    /// - 配置错误 → 2（Phase 4 引入 `Config` 变体后扩展）
    /// - `PathPolicy` / `CommandPolicy` / `NeedsConfirmation` → 3
    /// - 其它领域错误 → 1
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::PathPolicy(_) | Self::CommandPolicy(_) | Self::NeedsConfirmation(_) => 3,
            _ => 1,
        }
    }
}
