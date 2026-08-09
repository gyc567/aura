//! Session layer integration tests.

use std::path::PathBuf;
use std::sync::Arc;

use aura::domain::Message;
use aura::session::Session;
use aura::session::transcript::{InMemoryTranscript, JsonlTranscript, Transcript};

fn make_msg(content: &str) -> Message {
    Message::User {
        content: content.to_string(),
    }
}

#[test]
fn in_memory_transcript_append_and_replay() {
    let transcript = InMemoryTranscript::new();
    let msg1 = make_msg("hello");
    let msg2 = make_msg("world");

    transcript.append(msg1.clone()).unwrap();
    transcript.append(msg2.clone()).unwrap();

    let replayed = transcript.replay();
    assert_eq!(replayed.len(), 2);
    assert_eq!(replayed[0], msg1);
    assert_eq!(replayed[1], msg2);
}

#[test]
fn in_memory_transcript_multiple_append_order() {
    let transcript = InMemoryTranscript::new();
    let msgs: Vec<Message> = (0..5).map(|i| make_msg(&format!("msg_{i}"))).collect();

    for msg in &msgs {
        transcript.append(msg.clone()).unwrap();
    }

    let replayed = transcript.replay();
    assert_eq!(replayed, msgs);
}

#[test]
fn jsonl_transcript_replay_empty_for_missing_file() {
    let transcript = JsonlTranscript::new("/nonexistent/path/does_not_exist.jsonl");
    let replayed = transcript.replay();
    assert!(replayed.is_empty());
}

#[test]
fn session_new_creates_empty_messages() {
    let ws = PathBuf::from("/tmp/test");
    let session = Session::new(ws.clone(), Some("gpt4".to_string()));

    assert!(session.messages().is_empty());
    assert_eq!(session.meta.workspace, ws);
    assert_eq!(session.meta.model, Some("gpt4".to_string()));
    assert!(!session.meta.session_id.is_empty());
}

#[test]
fn session_push_appends_to_messages_and_transcript() {
    let ws = PathBuf::from("/tmp/test");
    let mut session = Session::new(ws.clone(), None);
    let msg1 = make_msg("first");
    let msg2 = make_msg("second");

    session.push(msg1.clone()).unwrap();
    session.push(msg2.clone()).unwrap();

    assert_eq!(session.messages().len(), 2);
    assert_eq!(session.messages()[0], msg1);
    assert_eq!(session.messages()[1], msg2);
}

#[test]
fn session_replay_returns_all_pushed_messages() {
    let ws = PathBuf::from("/tmp/test");
    let mut session = Session::new(ws, None);
    let msg1 = make_msg("alpha");
    let msg2 = make_msg("beta");

    session.push(msg1.clone()).unwrap();
    session.push(msg2.clone()).unwrap();

    let replayed = session.replay();
    assert_eq!(replayed.len(), 2);
    assert_eq!(replayed[0], msg1);
    assert_eq!(replayed[1], msg2);
}

#[test]
fn session_with_transcript_injects_transcript() {
    let transcript = Arc::new(InMemoryTranscript::new());
    let ws = PathBuf::from("/workspace");
    let session =
        Session::with_transcript(transcript.clone(), ws.clone(), Some("claude".to_string()));

    assert_eq!(session.meta.workspace, ws);
    assert_eq!(session.meta.model, Some("claude".to_string()));
}

#[test]
fn session_messages_mut_returns_mutable_vec() {
    let ws = PathBuf::from("/tmp/test");
    let mut session = Session::new(ws, None);

    session.messages_mut().push(make_msg("direct"));
    // messages_mut bypasses transcript; verify push is needed for transcript
    assert_eq!(session.messages().len(), 1);
}

#[test]
fn session_meta_clone_works() {
    let ws = PathBuf::from("/tmp/test");
    let session = Session::new(ws.clone(), Some("model".to_string()));
    let meta_clone = session.meta.clone();

    assert_eq!(meta_clone.workspace, ws);
    assert_eq!(meta_clone.model, Some("model".to_string()));
}

#[test]
fn jsonl_transcript_appends_and_replays() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("transcript.jsonl");
    let transcript = JsonlTranscript::new(path.clone());

    let msg1 = Message::System {
        content: "hello".into(),
    };
    let msg2 = Message::User {
        content: "world".into(),
    };

    transcript.append(msg1.clone()).unwrap();
    transcript.append(msg2.clone()).unwrap();

    let replayed = transcript.replay();
    assert_eq!(replayed.len(), 2);
    assert_eq!(replayed[0], msg1);
    assert_eq!(replayed[1], msg2);

    // Verify file was written
    let file_content = std::fs::read_to_string(&path).unwrap();
    assert!(file_content.contains("hello"));
    assert!(file_content.contains("world"));
}

#[test]
fn session_resume_replays_jsonl_messages() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("resume.jsonl");
    let workspace = dir.path().join("workspace");

    // Write some messages to the JSONL file
    let transcript = JsonlTranscript::new(&path);
    transcript
        .append(Message::System {
            content: "system msg".into(),
        })
        .unwrap();
    transcript
        .append(Message::User {
            content: "user msg".into(),
        })
        .unwrap();

    // Resume: create session from the transcript file
    let session = Session::resume(path.clone(), workspace.clone(), Some("gpt-4".into()));
    assert_eq!(session.messages().len(), 2);
    assert_eq!(
        session.messages()[0],
        Message::System {
            content: "system msg".into()
        }
    );
    assert_eq!(
        session.messages()[1],
        Message::User {
            content: "user msg".into()
        }
    );
    assert_eq!(session.meta.model, Some("gpt-4".into()));
}

#[test]
fn jsonl_transcript_empty_file_replays_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.jsonl");
    let transcript = JsonlTranscript::new(&path);
    // Don't write anything
    let replayed = transcript.replay();
    assert!(replayed.is_empty());
}

#[test]
fn scratchpad_summary_injects_into_compaction() {
    use aura::compaction::compact;
    use aura::tools::scratchpad::ScratchpadTool;
    use aura::{Tool, ToolArgument, ToolContext, ToolInput};
    use serde_json::json;

    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().to_path_buf();

    // Write scratchpad entries via the tool
    let tool = ScratchpadTool::new(workspace.clone());
    let ctx = ToolContext::new("/tmp".into(), "c1");

    let args = ToolArgument::new(json!({
        "action": "set",
        "name": "progress",
        "value": "x".repeat(300)
    }));
    tool.execute(ToolInput::new(args), &ctx).unwrap();

    // Generate summary from workspace
    let summary = ScratchpadTool::summary(&workspace);
    assert!(summary.is_some());
    let s = summary.unwrap();
    assert!(s.contains("progress: 300B"));

    // Now compact with the scratchpad summary
    let msgs: Vec<Message> = (0..15).map(|i| make_msg(&format!("step {i}"))).collect();
    let ctx2 = compact(&msgs, Some(&s), 100, 10, false);

    // Verify scratchpad summary appears in the layered context
    assert!(ctx2.scratchpad_summary.is_some());
    assert!(ctx2.scratchpad_summary.unwrap().contains("progress: 300B"));
}

#[test]
fn scratchpad_summary_none_when_no_file_during_compaction() {
    use aura::compaction::compact;
    use aura::tools::scratchpad::ScratchpadTool;

    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().to_path_buf();

    // No scratchpad.json exists
    let summary = ScratchpadTool::summary(&workspace);
    assert!(summary.is_none());

    // Compact with None scratchpad summary
    let msgs: Vec<Message> = (0..5).map(|i| make_msg(&format!("msg {i}"))).collect();
    let ctx = compact(&msgs, None, 100, 10, false);
    assert!(ctx.scratchpad_summary.is_none());
}
#[test]
fn session_scratchpad_summary_reads_file() {
    use aura::session::Session;
    use std::sync::Arc;

    let dir = tempfile::tempdir().unwrap();
    let session = Session::with_transcript(
        Arc::new(aura::session::transcript::InMemoryTranscript::new()),
        dir.path().to_path_buf(),
        None,
    );

    // No scratchpad file yet → None
    assert!(session.scratchpad_summary().is_none());

    // Write a scratchpad.json file directly
    let sp_path = session.artifacts_dir().join("scratchpad.json");
    std::fs::create_dir_all(session.artifacts_dir()).unwrap();
    std::fs::write(
        &sp_path,
        r#"{"notes": {"value": "hello world", "updated_at": 0}}"#,
    )
    .unwrap();

    let summary = session.scratchpad_summary();
    assert!(summary.is_some());
    let s = summary.as_deref().unwrap();
    assert!(s.contains("notes"));
    assert!(s.contains("11B")); // "hello world" = 11 bytes
}

#[test]
fn session_artifacts_dir_is_workspace_artifacts() {
    use aura::session::Session;
    use std::sync::Arc;

    let dir = tempfile::tempdir().unwrap();
    let session = Session::with_transcript(
        Arc::new(aura::session::transcript::InMemoryTranscript::new()),
        dir.path().to_path_buf(),
        None,
    );
    assert_eq!(session.artifacts_dir(), dir.path().join("artifacts"));
}

#[test]
fn session_compact_messages_replaces_early_with_summary() {
    let mut session = Session::new(PathBuf::from("/tmp"), None);
    session
        .push(Message::System {
            content: "sys".into(),
        })
        .unwrap();
    session.push(make_msg("u1")).unwrap();
    session.push(make_msg("u2")).unwrap();

    // 模拟 compact() 的 core_window：系统消息 + 最近 1 条
    let core = vec![
        Message::System {
            content: "sys".into(),
        },
        make_msg("u2"),
    ];
    session.compact_messages("earlier work summarized", &core);

    assert_eq!(session.messages().len(), 3);
    assert!(
        matches!(&session.messages()[0], Message::Assistant { content } if content.contains("earlier context summarized")),
        "first message is the summary"
    );
    assert!(matches!(&session.messages()[1], Message::System { .. }));
    assert_eq!(session.messages()[2], make_msg("u2"));
}
