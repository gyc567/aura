//! subagent spawn 端到端测试（State 待办：完整执行路径）。
//!
//! 覆盖：
//! - `subagent` 工具成功 spawn：子代理后台运行、独立 workspace、JSONL transcript 落盘
//! - 子代理执行工具循环（`todo_write`）
//! - `subagent_result` 工具：running / completed 状态与结果收集
//! - `subagent_result` 错误分支：空 id / 未知 child / 参数类型错误

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aura::children::ChildRegistry;
use aura::error::AgentError;
use aura::model::{ModelGateway, ModelRequest, ModelResponse};
use aura::tools::agent_message::AgentMessageTool;
use aura::tools::subagent::SubagentTool;
use aura::tools::subagent_result::SubagentResultTool;
use aura::{
    Budget, ChildStatus, Decision, ErrorBudget, InMemoryRegistry, Tool, ToolArgument, ToolContext,
    ToolInput, ToolOutput, VecEventSink,
};
use serde_json::json;
use tempfile::TempDir;

/// 按任务指令分发 parent/child 决策队列的假模型。
///
/// `ModelRequest.system` 携带任务指令（`agent::run` 将 `task.instruction`
/// 作为 system 传入）；含 `CHILD_MARKER` 的走 child 队列，否则走 parent 队列。
struct ScriptedModel {
    parent: Mutex<Vec<Decision>>,
    child: Mutex<Vec<Decision>>,
}

impl ScriptedModel {
    fn new(parent: Vec<Decision>, child: Vec<Decision>) -> Self {
        Self {
            parent: Mutex::new(parent),
            child: Mutex::new(child),
        }
    }

    fn pop(queue: &Mutex<Vec<Decision>>) -> Decision {
        let mut guard = queue.lock().unwrap();
        if guard.is_empty() {
            Decision::Done {
                summary: "noop".into(),
            }
        } else {
            guard.remove(0)
        }
    }
}

impl ModelGateway for ScriptedModel {
    fn complete(
        &self,
        req: ModelRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ModelResponse, AgentError>> + Send + '_>,
    > {
        // 任务指令现在作为 User message 出现在 messages 中（agent.rs 修复后 system 为空）。
        let is_child = req.messages.iter().any(
            |m| matches!(m, aura::Message::User { content } if content.contains("CHILD_MARKER")),
        );
        let decision = if is_child {
            Self::pop(&self.child)
        } else {
            Self::pop(&self.parent)
        };
        Box::pin(async move {
            Ok(ModelResponse {
                raw: format!("{decision:?}"),
                decision,
            })
        })
    }
}

/// 轮询直到子代理进入期望状态或超时。
fn wait_until<F: Fn() -> bool>(f: F, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    f()
}

/// 构建带 `subagent` / `agent_message` / `subagent_result` 的注册表脚手架。
struct Scaffold {
    model: Arc<ScriptedModel>,
    registry: Arc<ChildRegistry>,
    tools: Arc<InMemoryRegistry>,
}

fn scaffold(parent: Vec<Decision>, child: Vec<Decision>) -> Scaffold {
    let model: Arc<ScriptedModel> = Arc::new(ScriptedModel::new(parent, child));
    let registry = Arc::new(ChildRegistry::new());
    let tool_registry_ref: Arc<Mutex<Option<Arc<InMemoryRegistry>>>> = Arc::new(Mutex::new(None));
    let tools = Arc::new(InMemoryRegistry::new(vec![
        Arc::new(SubagentTool::new(
            model.clone(),
            registry.clone(),
            tool_registry_ref.clone(),
            2,
        )),
        Arc::new(AgentMessageTool::new(registry.clone())),
        Arc::new(SubagentResultTool::new(registry.clone())),
    ]));
    *tool_registry_ref.lock().unwrap() = Some(tools.clone());
    Scaffold {
        model,
        registry,
        tools,
    }
}

fn subagent_call(task: &str, name: &str) -> Decision {
    Decision::Call(
        aura::ToolCall::new(
            "c1",
            "subagent",
            ToolArgument::new(json!({ "task": task, "name": name })),
        )
        .expect("valid subagent call"),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn subagent_spawn_end_to_end() {
    let ws: TempDir = tempfile::tempdir().expect("tempdir");
    let sc = scaffold(
        vec![
            subagent_call("CHILD_MARKER: build a widget", "worker"),
            Decision::Done {
                summary: "parent done".into(),
            },
        ],
        vec![Decision::Done {
            summary: "child says done".into(),
        }],
    );

    let task = aura::TaskRequest::new("PARENT_TASK: coordinate", ws.path().to_path_buf(), 10)
        .expect("valid task");
    let mut sink = VecEventSink::new();
    let report = aura::run_agent(
        task,
        &*sc.model,
        &*sc.tools,
        Budget::new(10, 100_000).unwrap(),
        ErrorBudget::default(),
        &mut sink,
        Arc::new(AtomicBool::new(false)),
        None,
    )
    .await
    .expect("parent run");

    // 父代理正常完成，并 spawn 出 1 个子代理。
    assert!(matches!(
        report.stop_reason,
        aura::StopReasonPayload::Completed { .. }
    ));
    assert_eq!(sc.registry.len(), 1);

    // 子代理后台完成（轮询等待，异步）。
    let completed = wait_until(
        || {
            sc.registry
                .list()
                .iter()
                .any(|(_, status, _)| *status == ChildStatus::Completed)
        },
        Duration::from_secs(10),
    );
    assert!(completed, "child agent did not complete within 10s");

    let (cid, _, _) = sc.registry.list().pop().unwrap();
    let handle = sc.registry.get(&cid).expect("child handle");
    assert_eq!(handle.name.as_deref(), Some("worker"));
    let result = handle.result.expect("child result");
    assert!(
        result.contains("Task completed"),
        "unexpected child result: {result}"
    );

    // 独立 workspace + JSONL transcript 落盘（Architecture §4.2）。
    let session_dir = handle.session_dir;
    assert!(
        session_dir.join("workspace").is_dir(),
        "child workspace missing"
    );
    let transcript_path = session_dir.join(format!("{cid}.jsonl"));
    assert!(transcript_path.is_file(), "child transcript missing");
    let transcript = std::fs::read_to_string(&transcript_path).unwrap();
    assert!(
        transcript.lines().count() >= 1,
        "transcript should hold at least the system message, got: {transcript}"
    );
    assert!(
        transcript.contains("system-reminder"),
        "transcript should start with system message, got: {transcript}"
    );

    // 父代理用 subagent_result 收集结果（工具级验证，结果已就绪）。
    let tool = SubagentResultTool::new(sc.registry.clone());
    let out = tool
        .execute(
            ToolInput::new(ToolArgument::new(json!({ "child_id": cid.to_string() }))),
            &ToolContext::new(PathBuf::from("/tmp"), "c2"),
        )
        .expect("subagent_result executes");
    assert!(out.success);
    assert!(
        out.content.contains("\"status\":\"completed\""),
        "got: {}",
        out.content
    );
    assert!(
        out.content.contains("Task completed"),
        "got: {}",
        out.content
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn subagent_child_executes_tool_loop() {
    let ws: TempDir = tempfile::tempdir().expect("tempdir");
    let todo = aura::ToolCall::new(
        "tc1",
        "todo_write",
        ToolArgument::new(json!({
            "todos": [{ "id": "1", "content": "child plan", "status": "in_progress", "priority": "high" }]
        })),
    )
    .expect("valid todo call");
    let sc = scaffold(
        vec![
            subagent_call("CHILD_MARKER: plan the work", "planner"),
            Decision::Done {
                summary: "parent done".into(),
            },
        ],
        vec![
            Decision::Call(todo),
            Decision::Done {
                summary: "child planned".into(),
            },
        ],
    );

    let task =
        aura::TaskRequest::new("PARENT_TASK: go", ws.path().to_path_buf(), 10).expect("valid task");
    let mut sink = VecEventSink::new();
    aura::run_agent(
        task,
        &*sc.model,
        &*sc.tools,
        Budget::new(10, 100_000).unwrap(),
        ErrorBudget::default(),
        &mut sink,
        Arc::new(AtomicBool::new(false)),
        None,
    )
    .await
    .expect("parent run");

    let completed = wait_until(
        || {
            sc.registry
                .list()
                .iter()
                .any(|(_, status, _)| *status == ChildStatus::Completed)
        },
        Duration::from_secs(10),
    );
    assert!(completed, "child did not complete");

    let (cid, _, _) = sc.registry.list().pop().unwrap();
    let handle = sc.registry.get(&cid).unwrap();
    let result = handle.result.unwrap();
    assert!(
        result.contains("Task completed"),
        "unexpected child result: {result}"
    );

    // 子代理的工具调用（todo_write）进入其 transcript。
    let transcript =
        std::fs::read_to_string(handle.session_dir.join(format!("{cid}.jsonl"))).unwrap();
    assert!(
        transcript.contains("todo_write"),
        "child tool call missing from transcript: {transcript}"
    );
}

#[test]
fn subagent_result_error_branches() {
    let registry = Arc::new(ChildRegistry::new());
    let tool = SubagentResultTool::new(registry);
    let ctx = ToolContext::new(PathBuf::from("/tmp"), "c1");

    // 空 child_id
    let out = tool.execute(
        ToolInput::new(ToolArgument::new(json!({ "child_id": "  " }))),
        &ctx,
    );
    assert!(out.is_err());
    assert!(
        out.unwrap_err()
            .to_string()
            .contains("child_id must not be empty"),
        "empty id branch"
    );

    // 未知 child
    let out = tool.execute(
        ToolInput::new(ToolArgument::new(json!({ "child_id": "child_nope" }))),
        &ctx,
    );
    assert!(out.is_err());
    assert!(
        out.unwrap_err()
            .to_string()
            .contains("child agent not found"),
        "unknown id branch"
    );

    // 参数类型错误
    let out = tool.execute(
        ToolInput::new(ToolArgument::new(json!({ "child_id": 42 }))),
        &ctx,
    );
    assert!(out.is_err());
}

#[test]
fn subagent_result_reports_running_status() {
    let registry = Arc::new(ChildRegistry::new());
    let cid = registry.register(
        Some("slow".into()),
        PathBuf::from("/tmp/slow-child"),
        ChildStatus::Running,
    );
    let tool = SubagentResultTool::new(registry);
    let out: ToolOutput = tool
        .execute(
            ToolInput::new(ToolArgument::new(json!({ "child_id": cid.to_string() }))),
            &ToolContext::new(PathBuf::from("/tmp"), "c1"),
        )
        .expect("running child queryable");
    assert!(out.success);
    assert!(
        out.content.contains("\"status\":\"running\""),
        "got: {}",
        out.content
    );
    assert!(
        out.content.contains("\"result\":null"),
        "running child must have null result, got: {}",
        out.content
    );
}

#[test]
fn subagent_spawn_rejects_empty_task_and_zero_depth() {
    let registry = Arc::new(ChildRegistry::new());
    let model: Arc<ScriptedModel> = Arc::new(ScriptedModel::new(vec![], vec![]));
    let tool_registry_ref: Arc<Mutex<Option<Arc<InMemoryRegistry>>>> = Arc::new(Mutex::new(None));
    let ctx = ToolContext::new(PathBuf::from("/tmp"), "c1");

    // 空 task
    let tool = SubagentTool::new(
        model.clone(),
        registry.clone(),
        tool_registry_ref.clone(),
        2,
    );
    let out = tool.execute(
        ToolInput::new(ToolArgument::new(json!({ "task": "   " }))),
        &ctx,
    );
    assert!(out.is_err());

    // 深度 0 → 不可用
    let tool = SubagentTool::new(
        model.clone(),
        registry.clone(),
        tool_registry_ref.clone(),
        0,
    );
    let out = tool.execute(
        ToolInput::new(ToolArgument::new(json!({ "task": "do it" }))),
        &ctx,
    );
    assert!(out.is_err());
    assert!(
        out.unwrap_err().to_string().contains("max_depth is 0"),
        "depth-0 branch"
    );
}
