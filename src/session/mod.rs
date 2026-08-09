//! Session: holds messages + metadata.

pub mod transcript;
pub use transcript::JsonlTranscript;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::Message;
use crate::session::transcript::Transcript;

/// Session metadata (no secrets).
#[derive(Debug, Clone)]
pub struct SessionMeta {
    /// Unique session identifier.
    pub session_id: String,
    /// Workspace directory path.
    pub workspace: PathBuf,
    /// Unix timestamp of session creation.
    pub created_at: u64,
    /// Model name used for this session, if known.
    pub model: Option<String>,
}

/// The session struct.
pub struct Session {
    /// Session metadata.
    pub meta: SessionMeta,
    /// In-memory message buffer. Kept in sync with `transcript`.
    messages: Vec<Message>,
    /// Append-only transcript backend.
    transcript: Arc<dyn Transcript>,
}
#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("meta", &self.meta)
            .field("messages", &self.messages)
            .finish()
    }
}

impl Session {
    /// Create a new session with an in-memory transcript.
    ///
    /// The session ID is derived from `SystemTime::now()` to avoid adding a `uuid` dep.
    ///
    /// # Panics
    ///
    /// Panics if the system clock is before the Unix epoch.
    #[must_use]
    pub fn new(workspace: PathBuf, model: Option<String>) -> Self {
        Self::with_transcript(
            Arc::new(transcript::InMemoryTranscript::new()),
            workspace,
            model,
        )
    }

    /// Create a session with a custom transcript backend.
    #[must_use]
    pub fn with_transcript(
        transcript: Arc<dyn Transcript>,
        workspace: PathBuf,
        model: Option<String>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before Unix epoch");
        let session_id = format!("session_{}", now.as_nanos());
        Self {
            meta: SessionMeta {
                session_id,
                workspace,
                created_at: now.as_secs(),
                model,
            },
            messages: Vec::new(),
            transcript,
        }
    }

    /// Returns a reference to the message buffer.
    #[must_use]
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Returns a mutable reference to the message buffer.
    ///
    /// Prefer [`Session::push`] to keep the transcript in sync.
    #[must_use]
    pub fn messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    /// Append a message to both the buffer and the transcript.
    pub fn push(&mut self, msg: Message) -> Result<(), AgentError> {
        self.messages.push(msg.clone());
        self.transcript.append(msg)
    }

    /// Replay all messages from the transcript.
    #[must_use]
    pub fn replay(&self) -> Vec<Message> {
        self.transcript.replay()
    }

    /// Create a session from an existing JSONL transcript file (resume support).
    ///
    /// Reads all messages from the transcript file and populates the in-memory
    /// buffer so the agent loop can continue from where it left off.
    ///
    /// # Panics
    ///
    /// Panics if the system clock is before the Unix epoch.
    #[must_use]
    pub fn resume(transcript_path: PathBuf, workspace: PathBuf, model: Option<String>) -> Self {
        let transcript: Arc<dyn Transcript> =
            Arc::new(transcript::JsonlTranscript::new(transcript_path));
        let mut session = Self::with_transcript(transcript, workspace, model);
        // Replay existing messages into the in-memory buffer
        let msgs = session.transcript.replay();
        session.messages.clear();
        session.messages = msgs;
        session
    }
}

use crate::error::AgentError;
