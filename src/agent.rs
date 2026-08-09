//! 智能体 while 循环驱动。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §4、§5.6。
//!
//! v1 关键不变量：
//! - 唯一驱动：`interrupted` 标志 / `budget.check_turns` / 模型返回非 `Call` / `?` 传播。
//! - `recorder.transition` 失败只记录不阻断（`let _ =`）。
//! - 工具错误**喂回模型**继续执行，由 `error_budget` 封顶（默认 3 次）。
//! - 并发模型：`tokio` 单线程运行时；`Arc<StateMachine>` + `Arc<AtomicBool>` 通过
//!   `Clone` 共享给 SIGINT handler。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::compaction::{compact, should_compact};
use crate::domain::{Decision, Message, TaskRequest};
use crate::error::AgentError;
use crate::event::{AgentEvent, EventSink};
use crate::model::{ModelGateway, ModelRequest, ModelResponse};
use crate::registry::ToolRegistry;
use crate::reminders::{RemindedOutput, SystemReminders};
use crate::session::Session;
use crate::state::{AgentState, Budget, ErrorBudget, StateMachine};

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
    /// 错误预算耗尽，工具执行循环终止。
    ToolFailed {
        /// 错误信息。
        message: String,
    },
    /// 用户中断（SIGINT）。
    UserAborted,
}

/// `Agent::run` 顶层循环（Session-aware v1.2）。
///
/// 接收 `&mut Session` 而非裸 `Vec<Message>`，通过 Session 统一管理消息历史、
/// transcript 持久化与子代理注册表。
///
/// # Errors
///
/// 见 [`RunReport::stop_reason`]；CLI 据此返回非零退出码。
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub async fn run_with_session<M, R, S>(
    task: TaskRequest,
    model: &M,
    registry: &R,
    budget: Budget,
    error_budget: ErrorBudget,
    session: &mut Session,
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
    // Ensure system message is in session (idempotent via replay check)
    if session.messages().is_empty() {
        let system_msg = Message::System {
            content: SystemReminders::baseline().join("\n"),
        };
        session
            .push(system_msg)
            .map_err(|e| AgentError::Context(format!("append system message: {e}")))?;
    }
    // Ensure the task instruction is present as a user message (idempotent via replay
    // check) — 真实模型需要指令出现在 messages 里；`ModelRequest.system` 只作备用。
    if !session
        .messages()
        .iter()
        .any(|m| matches!(m, Message::User { .. }))
    {
        session
            .push(Message::User {
                content: task.instruction.clone(),
            })
            .map_err(|e| AgentError::Context(format!("append instruction message: {e}")))?;
    }
    let mut used_turns: u32 = 0;
    let mut error_budget = error_budget;
    let start = Instant::now();
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
        if budget.check_wall_time(start.elapsed()).is_err() {
            stop_reason = StopReasonPayload::BudgetExhausted { used: used_turns };
            break;
        }

        // 分层上下文压缩（Phase 7 §4.4 / M1 写回）
        // 当上下文达到阈值时，将早期消息压缩为摘要写回 session（替换早期段，
        // 保留系统消息 + 核心窗口），既消除"每轮重复压缩"也让模型视图有界。
        let messages_for_model = if should_compact(session.messages(), budget.max_context_bytes) {
            let scratchpad_summary = session.scratchpad_summary();
            let ctx = compact(
                session.messages(),
                scratchpad_summary.as_deref(),
                budget.max_context_bytes,
                DEFAULT_CORE_WINDOW_SIZE,
                false, // 写回后 session 不再膨胀，无需 already_summarized 占位
            );
            if ctx.history_summary.is_some() {
                let summary = ctx.history_summary.clone().unwrap_or_default();
                session.compact_messages(&summary, &ctx.core_window);
            }
            ctx.into_model_messages()
        } else {
            session.messages().to_vec()
        };

        sink.emit(AgentEvent::ModelRequested);
        let req = ModelRequest::new(String::new(), messages_for_model)
            .with_tool_schemas(registry.schemas());
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
        // 记录 assistant 消息（携带 tool_calls）——OpenAI 兼容协议要求 tool result
        // 消息必须引用先前 assistant 消息中声明过的 tool id。
        session
            .push(Message::Assistant {
                content: format!("{call:?}"),
                tool_calls: vec![call.clone()],
            })
            .map_err(|e| AgentError::Context(format!("push assistant msg: {e}")))?;
        let _ = recorder.transition(AgentState::ExecutingTool);
        sink.emit(AgentEvent::ToolStarted {
            name: call.name.clone(),
        });

        let ctx = crate::tool::ToolContext::new(task.workspace.clone(), call.id.clone());
        let output = match registry.execute(&call, &ctx) {
            Ok(o) => o,
            Err(e) => {
                let exhausted = error_budget.record();
                let err_msg = e.to_string();
                sink.emit(AgentEvent::Failed {
                    error: err_msg.clone(),
                });
                if exhausted {
                    stop_reason = StopReasonPayload::ToolFailed { message: err_msg };
                    break;
                }
                // 错误回填：附加错误消息和恢复提醒，继续循环
                let reminder = SystemReminders::error_recovery(error_budget.remaining());
                let content = format!("Tool error: {err_msg}\n\n{}", reminder.join("\n"));
                session
                    .push(Message::Tool {
                        call_id: call.id.clone(),
                        output: content,
                        success: false,
                    })
                    .map_err(|e| AgentError::Context(format!("push tool error msg: {e}")))?;
                used_turns = used_turns.saturating_add(1);
                continue;
            }
        };

        let reminded = RemindedOutput::wrap(&call.id, &call.name, output.clone());
        sink.emit(AgentEvent::ToolFinished {
            name: call.name.clone(),
            success: output.success,
        });
        session
            .push(Message::Tool {
                call_id: call.id.clone(),
                output: reminded.to_text(),
                success: output.success,
            })
            .map_err(|e| AgentError::Context(format!("push tool result msg: {e}")))?;
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

/// `Agent::run` 顶层循环（向后兼容包装器）。
///
/// 创建一个默认的 `Session`（内存 transcript） 并委托给 [`run_with_session`]。
///
/// # Errors
///
/// 见 [`RunReport::stop_reason`]；CLI 据此返回非零退出码。
#[allow(clippy::too_many_arguments)]
pub async fn run<M, R, S>(
    task: TaskRequest,
    model: &M,
    registry: &R,
    budget: Budget,
    error_budget: ErrorBudget,
    sink: &mut S,
    interrupted: Arc<AtomicBool>,
    model_name: Option<String>,
) -> Result<RunReport, AgentError>
where
    M: ModelGateway + ?Sized,
    R: ToolRegistry + ?Sized,
    S: EventSink,
{
    let mut session = Session::new(task.workspace.clone(), model_name);
    run_with_session(
        task,
        model,
        registry,
        budget,
        error_budget,
        &mut session,
        sink,
        interrupted,
    )
    .await
}

/// 默认核心窗口保留条数（与 `compaction.rs` 常量保持一致）。
const DEFAULT_CORE_WINDOW_SIZE: usize = 10;
