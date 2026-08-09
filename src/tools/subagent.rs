//! `subagent` 工具：RLM 式子代理 (Architecture §4.2)。
//!
//! 输入: `{ task, name?, model? }` → 立即返回 admission handle
//! `{ child_id, name, session_dir, status: "running" }`。
//!
//! 后台: `tokio::spawn` 子 agent 任务(独立消息历史、独立 transcript)。
//! `max_depth` 递减；深度 0 时返回错误。

use std::sync::{Arc, Mutex};

use crate::children::ChildRegistry;
use crate::domain::{ChildId, ChildStatus, TaskRequest};
use crate::error::AgentError;
use crate::model::ModelGateway;
use crate::registry::InMemoryRegistry;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};
use serde::Deserialize;

/// subagent 工具参数。
#[derive(Debug, Deserialize)]
struct SubagentInput {
    /// 子任务指令。
    task: String,
    /// 子代理名称（可选）。
    name: Option<String>,
    /// 模型名（可选，目前主要用于日志）。
    model: Option<String>,
}

/// `subagent` 工具。
///
/// 持有:
/// - `model`: 共享的模型网关（父子共用）
/// - `registry`: 子代理注册表（父子共享）
/// - `tool_registry`: 父代理的工具注册表克隆，用于子代理
/// - `max_depth`: 当前剩余递归深度
pub struct SubagentTool {
    model: Arc<dyn ModelGateway + Send + Sync>,
    registry: Arc<ChildRegistry>,
    tool_registry: Arc<Mutex<Option<Arc<InMemoryRegistry>>>>,
    max_depth: u32,
}

impl SubagentTool {
    /// 创建 subagent 工具。`tool_registry_ref` 在注册表构建完成后通过 `set_tool_registry` 设置。
    #[must_use]
    pub fn new(
        model: Arc<dyn ModelGateway + Send + Sync>,
        registry: Arc<ChildRegistry>,
        tool_registry: Arc<Mutex<Option<Arc<InMemoryRegistry>>>>,
        max_depth: u32,
    ) -> Self {
        Self {
            model,
            registry,
            tool_registry,
            max_depth,
        }
    }
}

impl Tool for SubagentTool {
    fn name(&self) -> &'static str {
        "subagent"
    }

    fn description(&self) -> &'static str {
        "Spawn a background sub-agent to work on a sub-task. Returns an admission handle with a child_id. The child runs independently; check its status via 'subagent_result'. Each sub-agent has its own isolated workspace and message history."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "Sub-task instruction for the child agent."
                    },
                    "name": {
                        "type": "string",
                        "description": "Optional name for the child agent."
                    },
                    "model": {
                        "type": "string",
                        "description": "Optional model override (currently uses parent's model)."
                    }
                },
                "required": ["task"],
                "additionalProperties": false
            }),
        }
    }

    fn execute(&self, input: ToolInput, ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        // Depth check: at depth 0, subagent is unavailable
        if self.max_depth == 0 {
            return Err(AgentError::InvalidRequest(
                "max_depth is 0; subagent tool is not available in this context".into(),
            ));
        }

        let args: SubagentInput = serde_json::from_value(input.arguments.into_value())
            .map_err(|e| AgentError::InvalidArguments(format!("subagent args: {e}")))?;

        if args.task.trim().is_empty() {
            return Err(AgentError::InvalidArguments(
                "task must not be empty".into(),
            ));
        }

        // Create child workspace
        let run_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let session_dir = ctx
            .workspace
            .join(format!("artifacts/children/child_{run_id}"));
        std::fs::create_dir_all(&session_dir)
            .map_err(|e| AgentError::Context(format!("create child session dir: {e}")))?;

        // Register child in registry
        let child_id =
            self.registry
                .register(args.name.clone(), session_dir.clone(), ChildStatus::Running);

        // Get tool registry for child (same as parent)
        let child_registry = self
            .tool_registry
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| AgentError::Context("tool registry not initialized".into()))?;

        // Create child task request with decremented depth
        let child_workspace_str = session_dir.join("workspace");
        std::fs::create_dir_all(&child_workspace_str)
            .map_err(|e| AgentError::Context(format!("create child workspace: {e}")))?;

        let child_task = TaskRequest::new_with_depth(
            args.task.clone(),
            child_workspace_str.clone(),
            12, // default max turns for child
            self.max_depth.saturating_sub(1),
        )
        .map_err(|e| AgentError::Context(e.to_string()))?;

        let child_id_str = child_id.to_string();
        let model_label = args.model.clone();

        // Spawn child agent in background
        let model = self.model.clone();
        let registry = self.registry.clone();
        let tool_registry = child_registry;
        let interrupted = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let session_dir_for_child = session_dir.clone();

        tokio::spawn(async move {
            let mut sink = crate::event::VecEventSink::new();
            let budget = crate::state::Budget::new(12, 100_000)
                .unwrap_or_else(|_| crate::state::Budget::new(12, 100_000).unwrap());
            let child_id_for_result = ChildId(child_id_str.clone());
            // 子会话 transcript 持久化到 artifacts/children/<child_id>.jsonl (Architecture §4.2)。
            let transcript_path = session_dir_for_child.join(format!("{child_id_str}.jsonl"));
            let mut session = crate::session::Session::with_transcript(
                Arc::new(crate::session::transcript::JsonlTranscript::new(
                    transcript_path,
                )),
                child_workspace_str.clone(),
                None,
            );
            let inbox =
                crate::children::ChildInbox::new(registry.clone(), child_id_for_result.clone());
            let result = crate::agent::run_with_session(
                child_task,
                &*model,
                &*tool_registry,
                budget,
                crate::state::ErrorBudget::default(),
                &mut session,
                &mut sink,
                interrupted,
                Some(inbox),
            )
            .await;

            let (is_ok, summary) = match result {
                Ok(report) => (
                    true,
                    format!(
                        "Task completed in {} turns. Stop: {:?}",
                        report.used_turns, report.stop_reason
                    ),
                ),
                Err(e) => (false, format!("Task failed: {e}")),
            };
            registry.set_result(&child_id_for_result, summary);
            if !is_ok {
                registry.set_status(&child_id_for_result, ChildStatus::Failed);
            }
        });

        // Return admission handle
        let handle_json = serde_json::json!({
            "child_id": child_id,
            "name": args.name.unwrap_or_else(|| child_id.to_string()),
            "session_dir": session_dir,
            "status": "running",
            "model": model_label.unwrap_or_else(|| "parent".into()),
        });

        Ok(ToolOutput::ok(handle_json.to_string()))
    }
}
