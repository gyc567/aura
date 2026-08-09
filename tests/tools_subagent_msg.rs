//! `subagent` / `agent_message` 工具分支测试（Phase 5 覆盖率补齐）。

use aura::children::ChildRegistry;
use aura::tools::agent_message::AgentMessageTool;
use aura::tools::subagent::SubagentTool;
use aura::{Tool, ToolArgument, ToolContext, ToolInput};
use serde_json::json;

#[test]
fn agent_message_empty_recipient_errors() {
    let registry = std::sync::Arc::new(ChildRegistry::new());
    let tool = AgentMessageTool::new(registry);
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({ "to": "  ", "content": "hi" }));
    let out = tool.execute(ToolInput::new(args), &ctx);
    assert!(out.is_err(), "empty recipient must error");
}

#[test]
fn agent_message_unknown_child_errors() {
    let registry = std::sync::Arc::new(ChildRegistry::new());
    let tool = AgentMessageTool::new(registry);
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({ "to": "child_1", "content": "hi" }));
    let out = tool.execute(ToolInput::new(args), &ctx);
    assert!(out.is_err());
    let err = out.unwrap_err().to_string();
    assert!(err.contains("child agent not found"), "got: {err}");
}

#[test]
fn agent_message_known_child_sends() {
    let registry = std::sync::Arc::new(ChildRegistry::new());
    let child_id = registry.register(
        Some("test child".into()),
        "/tmp/sess".into(),
        aura::ChildStatus::Running,
    );
    let tool = AgentMessageTool::new(registry);
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({ "to": child_id.to_string(), "content": "ping" }));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert!(out.success);
    assert!(out.content.contains("Message sent"));
}

#[test]
fn agent_message_bad_json_errors() {
    let registry = std::sync::Arc::new(ChildRegistry::new());
    let tool = AgentMessageTool::new(registry);
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({ "to": 42 })); // 类型不符
    let out = tool.execute(ToolInput::new(args), &ctx);
    assert!(out.is_err());
}

#[test]
fn subagent_depth_zero_unavailable() {
    let registry = std::sync::Arc::new(ChildRegistry::new());
    let tool = SubagentTool::new(
        std::sync::Arc::new(super_fake_model()),
        registry,
        std::sync::Arc::new(std::sync::Mutex::new(None)),
        0, // max_depth = 0 → 不可用
    );
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({ "task": "do something" }));
    let out = tool.execute(ToolInput::new(args), &ctx);
    assert!(out.is_err());
    let err = out.unwrap_err().to_string();
    assert!(err.contains("max_depth is 0"), "got: {err}");
}

#[test]
fn subagent_empty_task_errors() {
    let registry = std::sync::Arc::new(ChildRegistry::new());
    let tool = SubagentTool::new(
        std::sync::Arc::new(super_fake_model()),
        registry,
        std::sync::Arc::new(std::sync::Mutex::new(None)),
        1,
    );
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({ "task": "   " }));
    let out = tool.execute(ToolInput::new(args), &ctx);
    assert!(out.is_err());
}

// subagent 需要的 model：用最简 fake（不真正调用也行——上面分支在调用 model 前就返回）。
fn super_fake_model() -> impl aura::model::ModelGateway {
    struct M;
    impl aura::model::ModelGateway for M {
        fn complete(
            &self,
            _req: aura::model::ModelRequest,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<aura::model::ModelResponse, aura::error::AgentError>,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async {
                Err(aura::error::AgentError::Context(
                    "should not be called".into(),
                ))
            })
        }
    }
    M
}
