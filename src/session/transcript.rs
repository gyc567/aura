//! Transcript trait + in-memory + JSONL implementations.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::PathBuf;

use crate::domain::Message;
use crate::error::AgentError;

/// Append-only transcript of messages.
pub trait Transcript: Send + Sync {
    /// Append a message to the transcript.
    fn append(&self, msg: Message) -> Result<(), AgentError>;

    /// Replay all messages from the transcript.
    fn replay(&self) -> Vec<Message>;
}

/// In-memory transcript — no persistence, for tests.
#[derive(Debug, Default)]
pub struct InMemoryTranscript {
    messages: std::sync::Mutex<Vec<Message>>,
}

impl InMemoryTranscript {
    /// Create a new empty in-memory transcript.
    #[must_use]
    pub fn new() -> Self {
        Self {
            messages: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl Transcript for InMemoryTranscript {
    fn append(&self, msg: Message) -> Result<(), AgentError> {
        self.messages.lock().unwrap().push(msg);
        Ok(())
    }

    fn replay(&self) -> Vec<Message> {
        self.messages.lock().unwrap().clone()
    }
}

/// JSONL file transcript — appends each message as a JSON line to a file.
#[derive(Debug)]
pub struct JsonlTranscript {
    path: PathBuf,
}

impl JsonlTranscript {
    /// Create a new JSONL transcript at the given path.
    ///
    /// The file is created if it does not exist; if it exists, messages are
    /// appended in append mode so existing content is preserved.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn inner_append(&self, msg: &Message) -> Result<(), AgentError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| AgentError::Context(format!("open transcript file: {e}")))?;

        let mut writer = std::io::BufWriter::new(file);
        serde_json::to_writer(&mut writer, msg)
            .map_err(|e| AgentError::Context(format!("serialize message: {e}")))?;
        writeln!(writer).map_err(|e| AgentError::Context(format!("write transcript line: {e}")))?;
        writer
            .flush()
            .map_err(|e| AgentError::Context(format!("flush transcript writer: {e}")))?;
        Ok(())
    }

    fn inner_replay(&self) -> Vec<Message> {
        if !self.path.exists() {
            return Vec::new();
        }
        let Ok(file) = File::open(&self.path) else {
            return Vec::new();
        };
        let reader = BufReader::new(file);
        reader
            .lines()
            .filter_map(|line| {
                let Ok(l) = line else {
                    return None;
                };
                serde_json::from_str(&l).ok()
            })
            .collect()
    }
}

impl Transcript for JsonlTranscript {
    fn append(&self, msg: Message) -> Result<(), AgentError> {
        self.inner_append(&msg)
    }

    fn replay(&self) -> Vec<Message> {
        self.inner_replay()
    }
}
