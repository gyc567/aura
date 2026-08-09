//! 领域类型集成测试。
//!
//! 覆盖 `TaskRequest`、`Message`、`Decision`、`ToolCall`、`ToolArgument`
//! 全部公开方法与校验分支。

use std::path::PathBuf;

use aura::{AgentError, Decision, Message, TaskRequest, ToolArgument, ToolCall};
use serde_json::json;

#[test]
fn task_request_validates_required_fields() {
    let req = TaskRequest::new("fix bug", PathBuf::from("/tmp"), 4).unwrap();
    assert_eq!(req.instruction, "fix bug");
    assert_eq!(req.workspace, PathBuf::from("/tmp"));
    assert_eq!(req.max_turns, 4);
    req.validate().unwrap();
}

#[test]
fn task_request_rejects_empty_instruction() {
    let err = TaskRequest::new("   ", PathBuf::from("/tmp"), 1).unwrap_err();
    assert!(matches!(err, AgentError::InvalidRequest(_)));
}

#[test]
fn task_request_rejects_zero_turns() {
    let err = TaskRequest::new("hi", PathBuf::from("/tmp"), 0).unwrap_err();
    assert!(matches!(err, AgentError::InvalidRequest(_)));
}

#[test]
fn task_request_rejects_relative_workspace() {
    let err = TaskRequest::new("hi", PathBuf::from("rel/path"), 1).unwrap_err();
    assert!(matches!(err, AgentError::InvalidRequest(_)));
}

#[test]
fn decision_validate_call_delegates() {
    let call = ToolCall::new("c1", "read_file", ToolArgument::empty()).unwrap();
    Decision::Call(call).validate().unwrap();
}

#[test]
fn decision_validate_ask() {
    Decision::Ask {
        question: "?".into(),
    }
    .validate()
    .unwrap();
    let err = Decision::Ask {
        question: "  ".into(),
    }
    .validate()
    .unwrap_err();
    assert!(matches!(err, AgentError::UnparseableDecision(_)));
}

#[test]
fn decision_validate_done() {
    Decision::Done {
        summary: "ok".into(),
    }
    .validate()
    .unwrap();
    let err = Decision::Done {
        summary: String::new(),
    }
    .validate()
    .unwrap_err();
    assert!(matches!(err, AgentError::UnparseableDecision(_)));
}

#[test]
fn decision_validate_fail() {
    Decision::Fail {
        reason: "no".into(),
    }
    .validate()
    .unwrap();
    let err = Decision::Fail {
        reason: "\t".into(),
    }
    .validate()
    .unwrap_err();
    assert!(matches!(err, AgentError::UnparseableDecision(_)));
}

#[test]
fn tool_call_new_validates_id_and_name() {
    assert!(ToolCall::new("", "n", ToolArgument::empty()).is_err());
    assert!(ToolCall::new(" ", "n", ToolArgument::empty()).is_err());
    assert!(ToolCall::new("id", "", ToolArgument::empty()).is_err());
    assert!(ToolCall::new("id", " ", ToolArgument::empty()).is_err());
    let call = ToolCall::new("id", "name", ToolArgument::empty()).unwrap();
    assert_eq!(call.id, "id");
    assert_eq!(call.name, "name");
    call.validate().unwrap();
}

#[test]
fn tool_argument_helpers_and_from() {
    let empty = ToolArgument::empty();
    assert_eq!(
        empty.as_value(),
        &serde_json::Value::Object(serde_json::Map::default())
    );
    assert!(empty.clone().into_value().as_object().unwrap().is_empty());
    assert_eq!(ToolArgument::default(), ToolArgument::empty());
    let v: ToolArgument = json!({"a": 1}).into();
    assert_eq!(v.as_value(), &json!({"a": 1}));
}

#[test]
fn messages_serde_round_trip() {
    let msgs = vec![
        Message::System {
            content: "sys".into(),
        },
        Message::User {
            content: "u".into(),
        },
        Message::Assistant {
            content: "a".into(),
            tool_calls: Vec::new(),
        },
        Message::Tool {
            call_id: "c".into(),
            output: "o".into(),
            success: true,
        },
    ];
    let raw = serde_json::to_string(&msgs).unwrap();
    let back: Vec<Message> = serde_json::from_str(&raw).unwrap();
    assert_eq!(msgs, back);
}
