//! 上下文收集与截断集成测试。

use std::path::Path;

use aura::Message;
use aura::context::{ContextPriority, is_sensitive, truncate_messages};

#[test]
fn env_is_sensitive() {
    assert!(is_sensitive(Path::new("/workspace/.env")));
    assert!(is_sensitive(Path::new(".env")));
    assert!(is_sensitive(Path::new("/workspace/.envrc")));
}

#[test]
fn ssh_dir_is_sensitive() {
    assert!(is_sensitive(Path::new("/home/u/.ssh")));
    assert!(is_sensitive(Path::new("/workspace/.ssh/id_rsa")));
}

#[test]
fn secrets_or_credentials_dirs_are_sensitive() {
    assert!(is_sensitive(Path::new("/workspace/secrets/db")));
    assert!(is_sensitive(Path::new("/workspace/credentials/aws")));
}

#[test]
fn pem_or_key_pfx_extensions_are_sensitive() {
    assert!(is_sensitive(Path::new("/workspace/cert.pem")));
    assert!(is_sensitive(Path::new("/workspace/key.key")));
    assert!(is_sensitive(Path::new("/workspace/cert.pfx")));
}

#[test]
fn normal_source_files_are_not_sensitive() {
    assert!(!is_sensitive(Path::new("/workspace/src/main.rs")));
    assert!(!is_sensitive(Path::new("/workspace/Cargo.toml")));
    assert!(!is_sensitive(Path::new("/workspace/README.md")));
}

#[test]
fn priority_ordering_system_highest() {
    assert!(ContextPriority::System < ContextPriority::UserInstruction);
    assert!(ContextPriority::UserInstruction < ContextPriority::RecentTool);
    assert!(ContextPriority::RecentTool < ContextPriority::Early);
}

#[test]
fn truncate_no_op_when_under_budget() {
    let msgs = vec![
        Message::System {
            content: "sys".into(),
        },
        Message::User {
            content: "do it".into(),
        },
    ];
    let r = truncate_messages(msgs.clone(), 10_000).unwrap();
    assert_eq!(r.original_bytes, r.truncated_bytes);
    assert_eq!(r.dropped, 0);
    assert_eq!(r.messages.len(), msgs.len());
}

#[test]
fn truncate_drops_early_messages_when_over_budget() {
    // 构造：1 System + 1 UserInstruction + 5 Tool + 5 Assistant = 12 条消息
    let mut msgs = Vec::new();
    msgs.push(Message::System {
        content: "sys".into(),
    });
    msgs.push(Message::User {
        content: "do it".into(),
    });
    for i in 0..5 {
        msgs.push(Message::Tool {
            call_id: format!("c{i}"),
            output: format!("out{i}"),
            success: true,
        });
    }
    for i in 0..5 {
        msgs.push(Message::Assistant {
            content: format!("assistant {i}"),
            tool_calls: Vec::new(),
        });
    }
    let total_before = msgs.iter().map(message_len).sum::<u64>();
    let max = total_before / 2; // 只允许一半字节
    let r = truncate_messages(msgs, max).unwrap();
    assert!(r.truncated_bytes <= max);
    assert!(r.dropped > 0);
    // System 与 UserInstruction 必保留
    assert!(matches!(r.messages[0], Message::System { .. }));
    assert!(matches!(r.messages[1], Message::User { .. }));
}

#[test]
fn truncate_keeps_most_recent_tool_results() {
    // 10 条 Tool，最新 5 条应该保留
    let mut msgs = Vec::new();
    for i in 0..10 {
        msgs.push(Message::Tool {
            call_id: format!("c{i}"),
            output: format!("OUTPUT_{i:02}_PADDING_PADDING_PADDING"),
            success: true,
        });
    }
    let total: u64 = msgs.iter().map(message_len).sum();
    let max = total / 2;
    let r = truncate_messages(msgs, max).unwrap();
    // 至少应有 5 条 Tool 结果
    let tool_count = r
        .messages
        .iter()
        .filter(|m| matches!(m, Message::Tool { .. }))
        .count();
    assert!(tool_count <= 5);
    // 保留的应该是最近的 (c05..c09)
    let kept_ids: Vec<String> = r
        .messages
        .iter()
        .filter_map(|m| {
            if let Message::Tool { call_id, .. } = m {
                Some(call_id.clone())
            } else {
                None
            }
        })
        .collect();
    for kept_id in &kept_ids {
        let n: usize = kept_id.trim_start_matches('c').parse().unwrap();
        assert!(n >= 5);
    }
}

fn message_len(m: &Message) -> u64 {
    match m {
        Message::System { content }
        | Message::User { content }
        | Message::Assistant { content, .. } => content.len() as u64,
        Message::Tool { output, .. } => output.len() as u64,
    }
}

#[test]
fn truncate_zero_budget_keeps_only_system_and_instruction() {
    let msgs = vec![
        Message::System {
            content: "sys".into(),
        },
        Message::User {
            content: "instruction".into(),
        },
        Message::Assistant {
            content: "early".into(),
            tool_calls: Vec::new(),
        },
    ];
    let r = truncate_messages(msgs, 1).unwrap();
    // System 与 UserInstruction 必保留
    assert_eq!(r.messages.len(), 2);
    assert!(matches!(r.messages[0], Message::System { .. }));
    assert!(matches!(r.messages[1], Message::User { .. }));
}

#[test]
fn collect_workspace_files_reads_real_files() {
    let tmp = tempdir();
    std::fs::write(tmp.join("hello.txt"), "hello world").unwrap();
    let files = aura::context::collect_workspace_files(&tmp, &[tmp.join("hello.txt")]).unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].content.contains("hello"));
    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn collect_workspace_files_refuses_sensitive_path() {
    let tmp = tempdir();
    std::fs::write(tmp.join(".env"), "SECRET=foo").unwrap();
    let err = aura::context::collect_workspace_files(&tmp, &[tmp.join(".env")]).unwrap_err();
    assert!(matches!(err, aura::AgentError::PathPolicy(_)));
    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn collect_workspace_files_io_error_wrapped() {
    let tmp = tempdir();
    let err = aura::context::collect_workspace_files(&tmp, &[tmp.join("missing.txt")]).unwrap_err();
    assert!(matches!(err, aura::AgentError::Context(_)));
    let _ = std::fs::remove_dir_all(tmp);
}

fn tempdir() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("aura-test-{nanos}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}
