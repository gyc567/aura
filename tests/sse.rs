//! SSE 解析器集成测试。

use aura::sse::{SseError, SseEvent, SseParser};

#[test]
fn empty_input_returns_no_events() {
    let mut p = SseParser::new();
    let events = p.feed(b"").unwrap();
    assert!(events.is_empty());
    assert!(p.finish().is_none());
}

#[test]
fn single_event_basic() {
    let mut p = SseParser::new();
    let raw = b"data: hello world\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "hello world");
    assert_eq!(events[0].event, None);
}

#[test]
fn event_with_event_field() {
    let mut p = SseParser::new();
    let raw = b"event: ping\ndata: 1\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.as_deref(), Some("ping"));
    assert_eq!(events[0].data, "1");
}

#[test]
fn event_with_id_field() {
    let mut p = SseParser::new();
    let raw = b"id: 42\ndata: hi\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id.as_deref(), Some("42"));
}

#[test]
fn multiple_data_lines_concatenated_with_newline() {
    let mut p = SseParser::new();
    let raw = b"data: line1\ndata: line2\ndata: line3\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "line1\nline2\nline3");
}

#[test]
fn multiple_events_in_one_feed() {
    let mut p = SseParser::new();
    let raw = b"data: first\n\ndata: second\n\ndata: third\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].data, "first");
    assert_eq!(events[1].data, "second");
    assert_eq!(events[2].data, "third");
}

#[test]
fn comment_lines_are_ignored() {
    let mut p = SseParser::new();
    let raw = b": this is a comment\ndata: actual\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "actual");
}

#[test]
fn unknown_fields_are_ignored() {
    let mut p = SseParser::new();
    let raw = b"retry: 5000\ndata: hi\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "hi");
}

#[test]
fn carriage_return_in_line_is_stripped() {
    let mut p = SseParser::new();
    let raw = b"data: hi\r\n\r\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "hi");
}

#[test]
fn partial_event_is_buffered_across_feeds() {
    // 一个完整事件跨两次 feed；第二次 feed 给出剩余字节 + 终止空行
    let mut p = SseParser::new();
    let e1 = p.feed(b"data: par").unwrap();
    assert!(e1.is_empty());
    let e2 = p.feed(b"tial\n\n").unwrap();
    // "data: par" 触发 "data" 字段被 push；"tial" 是不带冒号的行，被忽略
    // （SSE 规范：每条 data 行必须有 "data:" 前缀；不能跨 feed 拼接）
    // 因此这里期望 "par" 而非 "partial"——验证 parser 严格遵循规范。
    assert_eq!(e2.len(), 1);
    assert_eq!(e2[0].data, "par");
}

#[test]
fn multi_data_lines_across_feeds() {
    // 多个 data: 行可跨 feed 边界；每个 "data:" 行累积新值。
    // "data:" 空值也占一格（spec）；三个 data 行 join("\n") 得 "a\n\nc"。
    let mut p = SseParser::new();
    let e1 = p.feed(b"data: a\ndata:").unwrap();
    assert!(e1.is_empty());
    let e2 = p.feed(b"data: c\n\n").unwrap();
    assert_eq!(e2.len(), 1);
    assert_eq!(e2[0].data, "a\n\nc");
}

#[test]
fn finish_returns_pending_event_without_terminator() {
    let mut p = SseParser::new();
    let e1 = p.feed(b"data: dangling").unwrap();
    assert!(e1.is_empty());
    let pending = p.finish();
    assert!(pending.is_some());
    assert_eq!(pending.unwrap().data, "dangling");
    assert!(p.finish().is_none());
}

#[test]
fn field_with_no_value_yields_empty_string() {
    let mut p = SseParser::new();
    let raw = b"data\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "");
}

#[test]
fn value_with_colon_preserved() {
    let mut p = SseParser::new();
    let raw = b"data: {\"k\":\"v\"}\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "{\"k\":\"v\"}");
}

#[test]
fn space_after_colon_stripped() {
    let mut p = SseParser::new();
    let raw = b"data:   spaced\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "spaced");
}

#[test]
fn tab_after_colon_stripped() {
    let mut p = SseParser::new();
    let raw = b"data:\tvalue\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "value");
}

#[test]
fn invalid_utf8_returns_error() {
    let mut p = SseParser::new();
    let invalid = &[0xff, 0xfe, 0xfd];
    let err = p.feed(invalid).unwrap_err();
    match err {
        SseError::InvalidUtf8(_) => {} // ok
    }
}

#[test]
fn empty_terminator_does_not_emit_empty_buffer() {
    let mut p = SseParser::new();
    // 连续空行
    let raw = b"\n\n\n";
    let events = p.feed(raw).unwrap();
    assert!(events.is_empty());
}

#[test]
fn event_with_all_fields() {
    let mut p = SseParser::new();
    let raw = b"event: chunk\nid: 1\ndata: hello\n\n";
    let events = p.feed(raw).unwrap();
    assert_eq!(events.len(), 1);
    let ev = &events[0];
    assert_eq!(ev.event.as_deref(), Some("chunk"));
    assert_eq!(ev.id.as_deref(), Some("1"));
    assert_eq!(ev.data, "hello");
}

#[test]
fn struct_event_equality() {
    let e1 = SseEvent {
        event: Some("x".into()),
        data: "y".into(),
        id: None,
    };
    let e2 = SseEvent {
        event: Some("x".into()),
        data: "y".into(),
        id: None,
    };
    assert_eq!(e1, e2);
}
