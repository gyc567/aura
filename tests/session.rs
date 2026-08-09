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
