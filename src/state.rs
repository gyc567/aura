//! 智能体状态机、预算与停止原因。
//!
//! 状态转移规则遵循设计文档 §5.5。状态机本身是纯数据：它不调用任何 IO，
//! 任何转移都会被合法性检查拒绝。

use serde::{Deserialize, Serialize};

use crate::error::AgentError;

/// 智能体当前状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    /// 空闲，等待任务。
    Ready,
    /// 模型决策中。
    Planning,
    /// 正在执行工具。
    ExecutingTool,
    /// 等待用户澄清。
    WaitingForUser,
    /// 运行验证流程（测试、lint）。
    Verifying,
    /// 任务完成。
    Completed,
    /// 任务失败。
    Failed,
}

impl AgentState {
    /// 状态是否终结（`Completed` 或 `Failed`）。
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }
}

/// 循环为何停止。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StopReason {
    /// 模型给出 `Done`。
    Completed {
        /// 摘要文本。
        summary: String,
    },
    /// 模型给出 `Fail`。
    ModelFailed {
        /// 失败原因。
        reason: String,
    },
    /// 轮次达到上限。
    TurnBudgetReached {
        /// 实际使用的轮次。
        used: u32,
    },
    /// 上下文字节数达到上限。
    ContextBudgetReached {
        /// 实际占用字节。
        used: u64,
    },
    /// 验证未通过。
    VerificationFailed {
        /// 错误信息。
        message: String,
    },
    /// 用户在交互流程中放弃。
    UserAborted,
}

/// 状态机非法转移时返回的错误。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid transition from {from:?} to {to:?}: {reason}")]
pub struct TransitionError {
    /// 起始状态。
    pub from: AgentState,
    /// 目标状态。
    pub to: AgentState,
    /// 拒绝原因。
    pub reason: &'static str,
}

impl From<TransitionError> for AgentError {
    fn from(err: TransitionError) -> Self {
        Self::InvalidTransition(err.to_string())
    }
}

/// 状态机。
///
/// 负责记录当前状态并校验转移合法性。状态机本身不驱动循环，循环由
/// 后续阶段的 `Agent` 编排。
#[derive(Debug, Clone)]
pub struct StateMachine {
    state: AgentState,
    last_reason: Option<&'static str>,
}

impl StateMachine {
    /// 创建处于 `Ready` 的状态机。
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AgentState::Ready,
            last_reason: None,
        }
    }

    /// 当前状态。
    #[must_use]
    pub fn state(&self) -> AgentState {
        self.state
    }

    /// 最近一次被拒绝的转移原因（如有）。
    #[must_use]
    pub fn last_rejection(&self) -> Option<&'static str> {
        self.last_reason
    }

    /// 尝试转移到目标状态。
    ///
    /// # Errors
    ///
    /// - [`TransitionError`]：当前状态下该转移非法。
    pub fn transition(&mut self, to: AgentState) -> Result<(), TransitionError> {
        if self.state.is_terminal() {
            self.last_reason = Some("current state is terminal");
            return Err(TransitionError {
                from: self.state,
                to,
                reason: "current state is terminal",
            });
        }
        if self.state == to {
            self.last_reason = Some("target equals current state");
            return Err(TransitionError {
                from: self.state,
                to,
                reason: "target equals current state",
            });
        }
        if !is_legal(self.state, to) {
            self.last_reason = Some("transition not allowed");
            return Err(TransitionError {
                from: self.state,
                to,
                reason: "transition not allowed",
            });
        }
        self.last_reason = None;
        self.state = to;
        Ok(())
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// 转移合法性表。集中维护，避免散落判断。
fn is_legal(from: AgentState, to: AgentState) -> bool {
    use AgentState::{
        Completed, ExecutingTool, Failed, Planning, Ready, Verifying, WaitingForUser,
    };
    matches!(
        (from, to),
        (Ready, Planning)
            | (
                Planning,
                ExecutingTool | WaitingForUser | Verifying | Failed
            )
            | (ExecutingTool | WaitingForUser, Planning | Failed)
            | (Verifying, Completed | Failed)
    )
}

/// 任务级预算。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budget {
    /// 最大循环轮次。
    pub max_turns: u32,
    /// 上下文最大字节数。
    pub max_context_bytes: u64,
}

impl Budget {
    /// 创建预算并校验不变式。
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidRequest`]：上限为 0。
    pub fn new(max_turns: u32, max_context_bytes: u64) -> Result<Self, AgentError> {
        if max_turns == 0 {
            return Err(AgentError::InvalidRequest(
                "budget max_turns must be greater than zero".into(),
            ));
        }
        if max_context_bytes == 0 {
            return Err(AgentError::InvalidRequest(
                "budget max_context_bytes must be greater than zero".into(),
            ));
        }
        Ok(Self {
            max_turns,
            max_context_bytes,
        })
    }

    /// 校验轮次是否仍在预算内。
    ///
    /// # Errors
    ///
    /// - [`AgentError::BudgetExhausted`]：`used >= max_turns`。
    pub fn check_turns(&self, used: u32) -> Result<(), AgentError> {
        if used >= self.max_turns {
            Err(AgentError::BudgetExhausted(format!(
                "used {used} turns, max {}",
                self.max_turns
            )))
        } else {
            Ok(())
        }
    }

    /// 校验上下文字节数是否仍在预算内。
    ///
    /// # Errors
    ///
    /// - [`AgentError::BudgetExhausted`]：`used > max_context_bytes`。
    pub fn check_context(&self, used: u64) -> Result<(), AgentError> {
        if used > self.max_context_bytes {
            Err(AgentError::BudgetExhausted(format!(
                "used {used} bytes, max {}",
                self.max_context_bytes
            )))
        } else {
            Ok(())
        }
    }
}
