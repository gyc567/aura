//! 工具注册表集成测试。

use std::sync::Arc;

use aura::{
    AgentError, InMemoryRegistry, Tool, ToolArgument, ToolCall, ToolContext, ToolInput, ToolOutput,
    ToolRegistry,
};
use serde_json::json;

struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &'static str {
        "echo"
    }
    fn description(&self) -> &'static str {
        "echoes arguments"
    }
    fn execute(&self, input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        Ok(ToolOutput::ok(input.arguments.as_value().to_string()))
    }
}

struct ConstTool;

impl Tool for ConstTool {
    fn name(&self) -> &'static str {
        "const"
    }
    fn description(&self) -> &'static str {
        "returns constant"
    }
    fn execute(&self, _input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        Ok(ToolOutput::ok("CONST"))
    }
}

#[test]
fn empty_registry_has_no_tools() {
    let reg = InMemoryRegistry::empty();
    assert!(reg.is_empty());
    assert_eq!(reg.len(), 0);
    assert!(!reg.contains("anything"));
}

#[test]
fn new_registers_all_tools() {
    let reg = InMemoryRegistry::new(vec![Arc::new(EchoTool), Arc::new(ConstTool)]);
    assert_eq!(reg.len(), 2);
    assert!(reg.contains("echo"));
    assert!(reg.contains("const"));
    assert!(!reg.contains("missing"));
}

#[test]
fn schemas_lists_all_tools() {
    let reg = InMemoryRegistry::new(vec![Arc::new(EchoTool), Arc::new(ConstTool)]);
    let schemas = reg.schemas();
    assert_eq!(schemas.len(), 2);
    let names: Vec<&str> = schemas.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"echo"));
    assert!(names.contains(&"const"));
}

#[test]
fn execute_dispatches_to_correct_tool() {
    let reg = InMemoryRegistry::new(vec![Arc::new(EchoTool), Arc::new(ConstTool)]);
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let call = ToolCall::new("c1", "echo", ToolArgument::new(json!({"k": "v"}))).unwrap();
    let out = reg.execute(&call, &ctx).unwrap();
    assert!(out.success);
    assert!(out.content.contains('k'));
}

#[test]
fn execute_returns_unknown_tool_error() {
    let reg = InMemoryRegistry::empty();
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let call = ToolCall::new("c1", "missing", ToolArgument::empty()).unwrap();
    let err = reg.execute(&call, &ctx).unwrap_err();
    assert!(matches!(err, AgentError::UnknownTool(ref n) if n == "missing"));
}

#[test]
fn execute_propagates_tool_error() {
    struct FailTool;
    impl Tool for FailTool {
        fn name(&self) -> &'static str {
            "fail"
        }
        fn description(&self) -> &'static str {
            "always errors"
        }
        fn execute(&self, _input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
            Err(AgentError::ToolFailed("boom".into()))
        }
    }
    let reg = InMemoryRegistry::new(vec![Arc::new(FailTool)]);
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let call = ToolCall::new("c1", "fail", ToolArgument::empty()).unwrap();
    let err = reg.execute(&call, &ctx).unwrap_err();
    assert!(matches!(err, AgentError::ToolFailed(_)));
}

#[test]
fn duplicate_tool_name_last_wins() {
    struct FirstTool;
    impl Tool for FirstTool {
        fn name(&self) -> &'static str {
            "dup"
        }
        fn description(&self) -> &'static str {
            "first"
        }
        fn execute(&self, _input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
            Ok(ToolOutput::ok("first"))
        }
    }
    struct SecondTool;
    impl Tool for SecondTool {
        fn name(&self) -> &'static str {
            "dup"
        }
        fn description(&self) -> &'static str {
            "second"
        }
        fn execute(&self, _input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
            Ok(ToolOutput::ok("second"))
        }
    }
    let reg = InMemoryRegistry::new(vec![Arc::new(FirstTool), Arc::new(SecondTool)]);
    assert_eq!(reg.len(), 1); // 后者覆盖前者
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let call = ToolCall::new("c1", "dup", ToolArgument::empty()).unwrap();
    let out = reg.execute(&call, &ctx).unwrap();
    assert_eq!(out.content, "second");
}
