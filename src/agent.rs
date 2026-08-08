//! 智能体 while 循环驱动。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §4、§5.6。
//!
//! v1 关键不变量：
//! - 唯一驱动：`interrupted` 标志 / `budget.check_turns` / 模型返回非 `Call` / `?` 传播。
//! - `recorder.transition` 失败只记录不阻断（`let _ =`）。
//! - 工具错误立即结束循环，**不喂回模型**。
//! - 并发模型：`tokio` 单线程运行时；`Arc<StateMachine>` + `Arc<AtomicBool>` 通过
//!   `Clone` 共享给 SIGINT handler。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::domain::{Decision, Message, TaskRequest};
use crate::error::AgentError;
use crate::event::{AgentEvent, EventSink};
use crate::model::{ModelGateway, ModelRequest, ModelResponse};
use crate::registry::ToolRegistry;
use crate::reminders::RemindedOutput;
use crate::reminders::SystemReminders;
use crate::state::{AgentState, Budget, StateMachine};

/// 一次 `run` 的最终报告。
#[derive(Debug, Clone)]
pub struct RunReport {
    /// 实际使用的轮次。
    pub used_turns: u32,
    /// 停止原因。
    pub stop_reason: StopReasonPayload,
    /// 最终 TODO 状态（Phase 2 起由 `todo_write` 写入；v1 通过 `event.jsonl` 重放推断）。
    /// 当前 v1 简化：`todo_write` 调用历史由调用方从 `EventSink` 重建。
    pub todo_final: Vec<()>,
}

/// 停止原因载荷。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReasonPayload {
    /// 模型给出 `Done` 或自然停止。
    Completed {
        /// 摘要。
        summary: String,
    },
    /// 模型给出 `Fail`。
    ModelFailed {
        /// 原因。
        reason: String,
    },
    /// 模型给出 `Ask`（v1 不在 Agent 内交互）。
    ModelAsked {
        /// 问题。
        question: String,
    },
    /// 轮次预算耗尽。
    BudgetExhausted {
        /// 已用轮次。
        used: u32,
    },
    /// 工具执行失败（v1 立即结束循环）。
    ToolFailed {
        /// 错误信息。
        message: String,
    },
    /// 用户中断（SIGINT）。
    UserAborted,
}

/// `Agent::run` 顶层循环。
///
/// # Errors
///
/// 见 [`RunReport::stop_reason`]；CLI 据此返回非零退出码。
#[allow(clippy::too_many_lines)]
pub async fn run<M, R, S>(
    task: TaskRequest,
    model: &M,
    registry: &R,
    budget: Budget,
    sink: &mut S,
    interrupted: Arc<AtomicBool>,
) -> Result<RunReport, AgentError>
where
    M: ModelGateway + ?Sized,
    R: ToolRegistry + ?Sized,
    S: EventSink,
{
    sink.emit(AgentEvent::Started {
        task: task.instruction.clone(),
    });

    let mut recorder = StateMachine::new();
    let mut messages: Vec<Message> = vec![Message::System {
        content: SystemReminders::baseline().join("\n"),
    }];
    let mut used_turns: u32 = 0;
    let stop_reason: StopReasonPayload;

    loop {
        if interrupted.load(Ordering::Relaxed) {
            stop_reason = StopReasonPayload::UserAborted;
            break;
        }

        if let Err(AgentError::BudgetExhausted(_)) = budget.check_turns(used_turns) {
            stop_reason = StopReasonPayload::BudgetExhausted { used: used_turns };
            break;
        }

        sink.emit(AgentEvent::ModelRequested);
        let req = ModelRequest::new(task.instruction.clone(), messages.clone());
        let resp: ModelResponse = model.complete(req).await?;

        // 终止条件：非 Call 即结束
        let call_opt = match resp.decision {
            Decision::Call(call) => Some(call),
            Decision::Ask { question } => {
                stop_reason = StopReasonPayload::ModelAsked { question };
                break;
            }
            Decision::Done { summary } => {
                stop_reason = StopReasonPayload::Completed { summary };
                break;
            }
            Decision::Fail { reason } => {
                stop_reason = StopReasonPayload::ModelFailed { reason };
                break;
            }
        };

        let call = call_opt.expect("Call branch is handled");
        let _ = recorder.transition(AgentState::ExecutingTool);
        sink.emit(AgentEvent::ToolStarted {
            name: call.name.clone(),
        });

        let ctx = crate::tool::ToolContext::new(task.workspace.clone(), call.id.clone());
        let output = match registry.execute(&call, &ctx) {
            Ok(o) => o,
            Err(e) => {
                stop_reason = StopReasonPayload::ToolFailed {
                    message: e.to_string(),
                };
                sink.emit(AgentEvent::Failed {
                    error: e.to_string(),
                });
                break;
            }
        };

        let reminded = RemindedOutput::wrap(&call.id, &call.name, output.clone());
        sink.emit(AgentEvent::ToolFinished {
            name: call.name.clone(),
            success: output.success,
        });
        messages.push(Message::Tool {
            call_id: call.id.clone(),
            output: reminded.to_text(),
            success: output.success,
        });
        used_turns = used_turns.saturating_add(1);
    }

    let _ = recorder.transition(AgentState::Completed);
    sink.emit(AgentEvent::Stopped {
        reason: match &stop_reason {
            StopReasonPayload::Completed { summary } => crate::state::StopReason::Completed {
                summary: summary.clone(),
            },
            StopReasonPayload::ModelFailed { reason } => crate::state::StopReason::ModelFailed {
                reason: reason.clone(),
            },
            StopReasonPayload::ModelAsked { .. } | StopReasonPayload::UserAborted => {
                crate::state::StopReason::UserAborted
            }
            StopReasonPayload::BudgetExhausted { used } => {
                crate::state::StopReason::TurnBudgetReached { used: *used }
            }
            StopReasonPayload::ToolFailed { message } => {
                crate::state::StopReason::VerificationFailed {
                    message: message.clone(),
                }
            }
        },
    });

    Ok(RunReport {
        used_turns,
        stop_reason,
        todo_final: Vec::new(),
    })
}
