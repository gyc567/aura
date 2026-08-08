//! `ModelStream` 和 `StreamEvent` 单元测试。
//!
//! 覆盖：`src/model.rs` 中 `ModelStream` 和 `StreamEvent` 类型的全部方法与变体。

use aura::domain::{Decision, Message};
use aura::model::{ModelRequest, ModelResponse, ModelStream, StreamEvent};

fn make_response(decision: Decision) -> ModelResponse {
    ModelResponse {
        decision,
        raw: "raw response text".into(),
    }
}

#[test]
fn model_stream_from_response_contains_complete_event() {
    let resp = make_response(Decision::Done {
        summary: "done".into(),
    });
    let stream = ModelStream::from_response(resp);
    assert_eq!(stream.len(), 1);
    assert!(!stream.is_empty());
}

#[test]
fn model_stream_into_events_consumes_stream() {
    let resp = make_response(Decision::Done {
        summary: "done".into(),
    });
    let stream = ModelStream::from_response(resp);
    let events = stream.into_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], StreamEvent::Complete(_)));
}

#[test]
fn model_stream_is_empty_false_when_has_events() {
    let resp = make_response(Decision::Done {
        summary: "done".into(),
    });
    let stream = ModelStream::from_response(resp);
    assert!(!stream.is_empty());
}

#[test]
fn model_stream_len_returns_event_count() {
    let resp = make_response(Decision::Done {
        summary: "done".into(),
    });
    let stream = ModelStream::from_response(resp);
    assert_eq!(stream.len(), 1);
}

#[test]
fn stream_event_complete_variant() {
    let resp = make_response(Decision::Done {
        summary: "test".into(),
    });
    let event = StreamEvent::Complete(resp);
    assert!(matches!(event, StreamEvent::Complete(_)));
}

#[test]
fn stream_event_delta_variant() {
    let event = StreamEvent::Delta("hello".into());
    assert!(matches!(event, StreamEvent::Delta(s) if s == "hello"));
}

#[test]
fn model_stream_clone_is_independent() {
    // Vec inside ModelStream, verify Clone works (required by EventSink)
    let resp = make_response(Decision::Done {
        summary: "done".into(),
    });
    let stream = ModelStream::from_response(resp);
    let cloned = stream.clone();
    assert_eq!(cloned.len(), stream.len());
}

#[test]
fn model_request_new_with_system_and_messages() {
    let msgs = vec![Message::User {
        content: "hello".into(),
    }];
    let req = ModelRequest::new("system prompt", msgs);
    assert_eq!(req.system, "system prompt");
    assert_eq!(req.messages.len(), 1);
    assert!(req.tool_schemas.is_empty());
}

#[test]
fn model_request_with_tool_schemas_appends_schemas() {
    let schema = aura::tool::ToolSchema {
        name: "test".into(),
        description: "test tool".into(),
        parameters: serde_json::json!({}),
    };
    let req2 = ModelRequest::new("sys", vec![]).with_tool_schemas(vec![schema.clone()]);
    assert_eq!(req2.tool_schemas.len(), 1);
    // original (unnamed) had empty schemas
    assert!(ModelRequest::new("sys", vec![]).tool_schemas.is_empty());
}
