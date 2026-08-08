//! `todo_write` 工具集成测试。

use std::sync::Arc;

use aura::tools::todo_write::TodoWriteTool;
use aura::{Tool, ToolArgument, ToolContext, ToolInput};
use serde_json::json;

#[test]
fn todo_write_starts_empty() {
    let tool = TodoWriteTool::new();
    assert!(tool.current().is_empty());
}

#[test]
fn todo_write_replaces_list_with_valid_input() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({
        "todos": [
            {"id": "1", "content": "Read", "status": "completed", "priority": "high"},
            {"id": "2", "content": "Edit", "status": "in_progress", "priority": "medium"},
            {"id": "3", "content": "Test", "status": "pending", "priority": "low"}
        ]
    }));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert!(out.success);
    assert!(out.content.contains("aura.todo.v1"));
    assert_eq!(tool.current().len(), 3);
}

#[test]
fn todo_write_replaces_existing_list() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::new("/tmp".into(), "c1");

    let first = ToolArgument::new(json!({
        "todos": [{"id": "1", "content": "A", "status": "pending", "priority": "high"}]
    }));
    tool.execute(ToolInput::new(first), &ctx).unwrap();
    assert_eq!(tool.current().len(), 1);

    let second = ToolArgument::new(json!({
        "todos": [
            {"id": "2", "content": "B", "status": "pending", "priority": "low"},
            {"id": "3", "content": "C", "status": "pending", "priority": "low"}
        ]
    }));
    tool.execute(ToolInput::new(second), &ctx).unwrap();
    assert_eq!(tool.current().len(), 2);
    // 旧 id="1" 不应再出现
    assert!(!tool.current().iter().any(|t| t.id == "1"));
}

#[test]
fn todo_write_rejects_missing_required_fields() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({
        "todos": [{"id": "1", "content": "x"}]
    }));
    let err = tool.execute(ToolInput::new(args), &ctx).unwrap_err();
    assert!(matches!(err, aura::AgentError::InvalidArguments(_)));
}

#[test]
fn todo_write_rejects_invalid_status() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({
        "todos": [{"id": "1", "content": "x", "status": "weird", "priority": "low"}]
    }));
    let err = tool.execute(ToolInput::new(args), &ctx).unwrap_err();
    assert!(matches!(err, aura::AgentError::InvalidArguments(_)));
}

#[test]
fn todo_write_rejects_invalid_priority() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({
        "todos": [{"id": "1", "content": "x", "status": "pending", "priority": "urgent"}]
    }));
    assert!(tool.execute(ToolInput::new(args), &ctx).is_err());
}

#[test]
fn todo_write_schema_has_required_fields() {
    let tool = TodoWriteTool::new();
    let schema = tool.schema();
    assert_eq!(schema.name, "todo_write");
    assert!(!schema.description.is_empty());
    let params = schema.parameters.as_object().expect("object schema");
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["todos"].is_object());
    assert!(
        params["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "todos")
    );
}

#[test]
fn todo_write_works_via_dyn_tool() {
    let tool: Arc<dyn Tool> = Arc::new(TodoWriteTool::new());
    assert_eq!(tool.name(), "todo_write");
    assert!(tool.required_capabilities().is_empty());
    assert!(!tool.needs_confirmation());
}

#[test]
fn todo_write_result_message_carries_version() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({
        "todos": [{"id": "1", "content": "x", "status": "pending", "priority": "high"}]
    }));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert!(out.content.contains("aura.todo.v1"));
    assert!(out.content.contains("1 items"));
}

#[test]
fn todo_write_empty_array_clears_list() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let first = ToolArgument::new(json!({
        "todos": [{"id": "1", "content": "x", "status": "pending", "priority": "high"}]
    }));
    tool.execute(ToolInput::new(first), &ctx).unwrap();
    assert_eq!(tool.current().len(), 1);

    let empty = ToolArgument::new(json!({"todos": []}));
    tool.execute(ToolInput::new(empty), &ctx).unwrap();
    assert!(tool.current().is_empty());
}
