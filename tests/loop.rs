//! `Agent::run` while 循环集成测试。
//!
//! 覆盖 v0.5 §8.1 列出的 6 类场景 + SIGINT 中断 + 事件顺序 + `todo_write` 集成。
//! 全部使用 `FakeModel` 模拟模型响应，不依赖网络。

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use serde_json::json;

use aura::domain::{Decision, ToolArgument, ToolCall};
use aura::error::AgentError;
use aura::event::{AgentEvent, VecEventSink};
use aura::model::{ModelGateway, ModelRequest, ModelResponse};
use aura::tools::todo_write::TodoWriteTool;
use aura::{
    Budget, ErrorBudget, InMemoryRegistry, StopReasonPayload, Tool, ToolContext, ToolInput,
    ToolOutput, run_agent,
};

/// `FakeModel`：按 FIFO 队列返回预置决策。
struct FakeModel {
    queue: Mutex<Vec<Decision>>,
}

impl FakeModel {
    fn new(decisions: Vec<Decision>) -> Self {
        Self {
            queue: Mutex::new(decisions),
        }
    }
}

impl ModelGateway for FakeModel {
    fn complete(
        &self,
        _request: ModelRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ModelResponse, AgentError>> + Send + '_>,
    > {
        let next = self.queue.lock().unwrap().remove(0);
        Box::pin(async move {
            Ok(ModelResponse {
                raw: format!("{next:?}"),
                decision: next,
            })
        })
    }
}

fn call(name: &str) -> ToolCall {
    ToolCall::new("c1", name, ToolArgument::empty()).unwrap()
}

struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &'static str {
        "echo"
    }
    fn description(&self) -> &'static str {
        "echoes"
    }
    fn execute(&self, _input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        Ok(ToolOutput::ok("ECHO"))
    }
}

struct FailTool;

impl Tool for FailTool {
    fn name(&self) -> &'static str {
        "fail"
    }
    fn description(&self) -> &'static str {
        "always fails"
    }
    fn execute(&self, _input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        Err(AgentError::ToolFailed("intentional".into()))
    }
}

fn task(workspace: std::path::PathBuf, max_turns: u32) -> aura::TaskRequest {
    aura::TaskRequest::new("do thing", workspace, max_turns).unwrap()
}

// 场景 1：成功完成（多轮 Call → Done）
#[tokio::test(flavor = "current_thread")]
async fn scenario_success_completes_after_two_turns() {
    let model = FakeModel::new(vec![
        Decision::Call(call("echo")),
        Decision::Call(call("echo")),
        Decision::Done {
            summary: "done".into(),
        },
    ]);
    let registry = InMemoryRegistry::new(vec![Arc::new(EchoTool)]);
    let budget = Budget::new(10, 100_000).unwrap();
    let mut sink = VecEventSink::new();
    let interrupted = Arc::new(AtomicBool::new(false));

    let report = run_agent(
        task("/tmp".into(), 10),
        &model,
        &registry,
        budget,
        ErrorBudget::default(),
        &mut sink,
        interrupted,
        None,
    )
    .await
    .unwrap();

    assert_eq!(report.used_turns, 2);
    assert!(matches!(
        report.stop_reason,
        StopReasonPayload::Completed { ref summary } if summary == "done"
    ));
}

// 场景 2：Ask 暂停
#[tokio::test(flavor = "current_thread")]
async fn scenario_ask_pause_returns_model_asked() {
    let model = FakeModel::new(vec![Decision::Ask {
        question: "What color?".into(),
    }]);
    let registry = InMemoryRegistry::empty();
    let budget = Budget::new(10, 100_000).unwrap();
    let mut sink = VecEventSink::new();
    let interrupted = Arc::new(AtomicBool::new(false));

    let report = run_agent(
        task("/tmp".into(), 10),
        &model,
        &registry,
        budget,
        ErrorBudget::default(),
        &mut sink,
        interrupted,
        None,
    )
    .await
    .unwrap();

    assert!(matches!(
        report.stop_reason,
        StopReasonPayload::ModelAsked { ref question } if question == "What color?"
    ));
    assert_eq!(report.used_turns, 0);
}

// 场景 3：Done 自然结束
#[tokio::test(flavor = "current_thread")]
async fn scenario_done_terminates_immediately() {
    let model = FakeModel::new(vec![Decision::Done {
        summary: "nothing to do".into(),
    }]);
    let registry = InMemoryRegistry::empty();
    let budget = Budget::new(10, 100_000).unwrap();
    let mut sink = VecEventSink::new();
    let interrupted = Arc::new(AtomicBool::new(false));

    let report = run_agent(
        task("/tmp".into(), 10),
        &model,
        &registry,
        budget,
        ErrorBudget::default(),
        &mut sink,
        interrupted,
        None,
    )
    .await
    .unwrap();

    assert!(matches!(
        report.stop_reason,
        StopReasonPayload::Completed { .. }
    ));
    assert_eq!(report.used_turns, 0);
}

// 场景 4：Fail 自然结束
#[tokio::test(flavor = "current_thread")]
async fn scenario_fail_terminates_with_model_failed() {
    let model = FakeModel::new(vec![Decision::Fail {
        reason: "cannot proceed".into(),
    }]);
    let registry = InMemoryRegistry::empty();
    let budget = Budget::new(10, 100_000).unwrap();
    let mut sink = VecEventSink::new();
    let interrupted = Arc::new(AtomicBool::new(false));

    let report = run_agent(
        task("/tmp".into(), 10),
        &model,
        &registry,
        budget,
        ErrorBudget::default(),
        &mut sink,
        interrupted,
        None,
    )
    .await
    .unwrap();

    assert!(matches!(
        report.stop_reason,
        StopReasonPayload::ModelFailed { ref reason } if reason == "cannot proceed"
    ));
}

// 场景 5：预算耗尽
#[tokio::test(flavor = "current_thread")]
async fn scenario_budget_exhausted() {
    let model = FakeModel::new(vec![
        Decision::Call(call("echo")),
        Decision::Call(call("echo")),
        Decision::Call(call("echo")),
        Decision::Call(call("echo")),
    ]);
    let registry = InMemoryRegistry::new(vec![Arc::new(EchoTool)]);
    let budget = Budget::new(2, 100_000).unwrap();
    let mut sink = VecEventSink::new();
    let interrupted = Arc::new(AtomicBool::new(false));

    let report = run_agent(
        task("/tmp".into(), 2),
        &model,
        &registry,
        budget,
        ErrorBudget::default(),
        &mut sink,
        interrupted,
        None,
    )
    .await
    .unwrap();

    assert_eq!(report.used_turns, 2);
    assert!(matches!(
        report.stop_reason,
        StopReasonPayload::BudgetExhausted { used: 2 }
    ));
}

// 场景 6：错误预算耗尽（ErrorBudget=1 时首次失败即停）
#[tokio::test(flavor = "current_thread")]
async fn scenario_tool_failure_terminates_loop() {
    let model = FakeModel::new(vec![
        Decision::Call(call("fail")),
        Decision::Done {
            summary: "unreached".into(),
        },
    ]);
    let registry = InMemoryRegistry::new(vec![Arc::new(FailTool)]);
    let budget = Budget::new(10, 100_000).unwrap();
    let mut sink = VecEventSink::new();
    let interrupted = Arc::new(AtomicBool::new(false));

    let report = run_agent(
        task("/tmp".into(), 10),
        &model,
        &registry,
        budget,
        ErrorBudget::new(1), // 首次失败即耗尽预算
        &mut sink,
        interrupted,
        None,
    )
    .await
    .unwrap();

    assert_eq!(report.used_turns, 0);
    assert!(matches!(
        report.stop_reason,
        StopReasonPayload::ToolFailed { .. }
    ));
}

// 场景 7：SIGINT 中断
#[tokio::test(flavor = "current_thread")]
async fn scenario_interrupt_aborts_cleanly() {
    let model = FakeModel::new(vec![Decision::Call(call("echo")); 100]);
    let registry = InMemoryRegistry::new(vec![Arc::new(EchoTool)]);
    let budget = Budget::new(100, 100_000).unwrap();
    let mut sink = VecEventSink::new();
    let interrupted = Arc::new(AtomicBool::new(true));

    let report = run_agent(
        task("/tmp".into(), 100),
        &model,
        &registry,
        budget,
        ErrorBudget::default(),
        &mut sink,
        interrupted,
        None,
    )
    .await
    .unwrap();

    assert_eq!(report.used_turns, 0);
    assert!(matches!(report.stop_reason, StopReasonPayload::UserAborted));
}

// 场景 8：事件顺序
#[tokio::test(flavor = "current_thread")]
async fn scenario_events_emitted_in_order() {
    let model = FakeModel::new(vec![
        Decision::Call(call("echo")),
        Decision::Done {
            summary: "x".into(),
        },
    ]);
    let registry = InMemoryRegistry::new(vec![Arc::new(EchoTool)]);
    let budget = Budget::new(10, 100_000).unwrap();
    let mut sink = VecEventSink::new();
    let interrupted = Arc::new(AtomicBool::new(false));

    let _ = run_agent(
        task("/tmp".into(), 10),
        &model,
        &registry,
        budget,
        ErrorBudget::default(),
        &mut sink,
        interrupted,
        None,
    )
    .await
    .unwrap();

    let events = sink.into_events();
    // 期望顺序：Started → [ModelRequested → ToolStarted → ToolFinished] → ModelRequested → Stopped
    // 两次 model 调用（第一次 Call → 第二次 Done → break）。
    assert_eq!(events.len(), 6);
    assert!(matches!(events[0], AgentEvent::Started { .. }));
    assert!(matches!(events[1], AgentEvent::ModelRequested));
    assert!(matches!(events[2], AgentEvent::ToolStarted { .. }));
    assert!(matches!(events[3], AgentEvent::ToolFinished { .. }));
    assert!(matches!(events[4], AgentEvent::ModelRequested));
    assert!(matches!(events[5], AgentEvent::Stopped { .. }));
}

// 场景 9：todo_write 集成
#[tokio::test(flavor = "current_thread")]
async fn scenario_todo_write_then_done() {
    let todos = json!({"todos": [
        {"id": "1", "content": "step 1", "status": "pending", "priority": "high"}
    ]});
    let todo_call = ToolCall::new("c1", "todo_write", ToolArgument::new(todos)).unwrap();
    let model = FakeModel::new(vec![
        Decision::Call(todo_call),
        Decision::Done {
            summary: "ok".into(),
        },
    ]);
    let todo_tool: Arc<dyn Tool> = Arc::new(TodoWriteTool::new());
    let registry = InMemoryRegistry::new(vec![todo_tool]);
    let budget = Budget::new(10, 100_000).unwrap();
    let mut sink = VecEventSink::new();
    let interrupted = Arc::new(AtomicBool::new(false));

    let report = run_agent(
        task("/tmp".into(), 10),
        &model,
        &registry,
        budget,
        ErrorBudget::default(),
        &mut sink,
        interrupted,
        None,
    )
    .await
    .unwrap();

    assert_eq!(report.used_turns, 1);
    assert!(matches!(
        report.stop_reason,
        StopReasonPayload::Completed { .. }
    ));
}
