//! # Aura 编码智能体（Phase 1：纯领域核心）
//!
//! 设计遵循 KISS 与高内聚低耦合：
//! - 领域类型保持纯数据与不变式，不接触 IO。
//! - 模型与工具仅以 trait 暴露，具体适配器在后续阶段加入。
//! - 所有公开行为由单元测试覆盖，目标 100% 行/分支覆盖。
//!
//! 参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md)。

#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod agent;
pub mod cli;
pub mod context;
pub mod domain;
pub mod error;
pub mod event;
pub mod model;
pub mod model_http;
pub mod output;
pub mod policy;
pub mod precheck;
pub mod registry;
pub mod reminders;
pub mod sse;
pub mod state;
pub mod tool;
pub mod tools;

pub use agent::{RunReport, StopReasonPayload, run as run_agent};
pub use context::{
    ContextFile, ContextPriority, TruncationResult, collect_workspace_files, is_sensitive,
    truncate_messages,
};
pub use domain::{Decision, Message, TaskRequest, ToolArgument, ToolCall};
pub use error::AgentError;
pub use event::{AgentEvent, EventSink, VecEventSink};
pub use model::{ModelGateway, ModelRequest, ModelResponse, ModelStream, StreamEvent};
pub use model_http::{HttpConfig, HttpModelAdapter};
pub use policy::{Capability, Policy, PolicyLevel};
pub use precheck::{PrecheckResult, RiskTier, analyze};
pub use registry::{InMemoryRegistry, ToolRegistry};
pub use reminders::{
    GLOBAL_REMINDERS, READ_ONLY_REMINDERS, RUN_COMMAND_REMINDERS, RemindedOutput, SystemReminders,
    TODO_WRITE_REMINDERS, TodoItem, TodoPriority, TodoStatus, WRITE_FILE_REMINDERS,
};
pub use sse::{SseError, SseEvent, SseParser};
pub use state::{AgentState, Budget, StateMachine, StopReason, TransitionError};
pub use tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};
