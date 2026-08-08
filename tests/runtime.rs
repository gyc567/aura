//! 事件、工具、模型接口集成测试。

use std::path::PathBuf;

use std::sync::Mutex;

use aura::{
    AgentEvent, Decision, EventSink, Message, ModelGateway, ModelRequest, ModelResponse, Tool,
    ToolArgument, ToolContext, ToolInput, ToolOutput, ToolSchema, VecEventSink,
};
use serde_json::json;

/// 用于模型测试的 `FakeModel`：按队列返回 `Decision`。
///
/// 使用 `std::sync::Mutex` 而非 `RefCell`，因为 `ModelGateway: Send + Sync`。
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
        _req: ModelRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ModelResponse, aura::AgentError>> + Send + '_>,
    > {
        let next = if self.queue.lock().unwrap().is_empty() {
            Decision::Done {
                summary: "noop".into(),
            }
        } else {
            self.queue.lock().unwrap().remove(0)
        };
        Box::pin(async move {
            Ok(ModelResponse {
                raw: format!("{next:?}"),
                decision: next,
            })
        })
    }
}

#[test]
fn vec_event_sink_collects_in_order() {
    let mut sink = VecEventSink::new();
    sink.emit(AgentEvent::Started { task: "t".into() });
    sink.emit(AgentEvent::ModelRequested);
    sink.emit(AgentEvent::Stopped {
        reason: aura::StopReason::UserAborted,
    });
    let events = sink.into_events();
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], AgentEvent::Started { .. }));
    assert_eq!(events[1], AgentEvent::ModelRequested);
}

#[test]
fn vec_event_sink_events_ref() {
    let mut sink = VecEventSink::new();
    sink.emit(AgentEvent::ModelRequested);
    assert_eq!(sink.events().len(), 1);
}

#[test]
fn tool_output_helpers() {
    let ok = ToolOutput::ok("hi");
    assert!(ok.success);
    assert_eq!(ok.content, "hi");
    let err = ToolOutput::err("oops");
    assert!(!err.success);
    assert_eq!(err.content, "oops");
}

#[test]
fn tool_input_and_context_constructors() {
    let ctx = ToolContext::new(PathBuf::from("/tmp"), "c1");
    assert_eq!(ctx.workspace, PathBuf::from("/tmp"));
    assert_eq!(ctx.call_id, "c1");
    let input = ToolInput::new(ToolArgument::empty());
    assert_eq!(input.arguments, ToolArgument::empty());
}

#[test]
fn tool_schema_construction() {
    let s = ToolSchema::new("read_file", "reads a file");
    assert_eq!(s.name, "read_file");
    assert_eq!(s.description, "reads a file");
}

/// 演示一个最小工具实现：只回显参数 JSON 文本。
struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn description(&self) -> &'static str {
        "echoes arguments back"
    }

    fn execute(
        &self,
        input: ToolInput,
        _ctx: &ToolContext,
    ) -> Result<ToolOutput, aura::AgentError> {
        Ok(ToolOutput::ok(input.arguments.as_value().to_string()))
    }
}

#[test]
fn tool_trait_default_schema_and_execute() {
    let tool = EchoTool;
    assert_eq!(tool.name(), "echo");
    assert_eq!(
        tool.schema(),
        ToolSchema::new("echo", "echoes arguments back")
    );
    let out = tool
        .execute(
            ToolInput::new(ToolArgument::new(json!({"k": 1}))),
            &ToolContext::new(PathBuf::from("/tmp"), "call-1"),
        )
        .unwrap();
    assert!(out.success);
    assert!(out.content.contains("\"k\":1"));
}

#[tokio::test]
async fn fake_model_returns_queued_decisions() {
    let model = FakeModel::new(vec![
        Decision::Call(aura::ToolCall::new("c1", "echo", ToolArgument::empty()).unwrap()),
        Decision::Done {
            summary: "ok".into(),
        },
    ]);
    let req = ModelRequest::new(
        "sys",
        vec![Message::User {
            content: "do".into(),
        }],
    );
    let first = model.complete(req.clone()).await.unwrap();
    assert!(matches!(first.decision, Decision::Call(_)));
    let second = model.complete(req).await.unwrap();
    assert!(matches!(second.decision, Decision::Done { .. }));
}
