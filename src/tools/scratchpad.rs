#![allow(clippy::cast_possible_wrap)]

//! `scratchpad` 工具：持久化键值存储。
//!
//! 数据保存在 `artifacts/scratchpad.json`，跨 agent turns 持久化。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::AgentError;
use crate::tool::{Tool, ToolContext, ToolInput, ToolOutput, ToolSchema};

/// 单条 scratchpad 条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchpadEntry {
    /// 条目内容。
    pub value: String,
    /// 更新时间戳（Unix seconds）。
    pub updated_at: i64,
}

/// scratchpad 内部状态（内存缓存 + 文件路径）。
pub(crate) struct ScratchpadStore {
    state: Mutex<HashMap<String, ScratchpadEntry>>,
    path: PathBuf,
}

impl ScratchpadStore {
    /// 从文件加载（或返回空 scratchpad）。
    pub(crate) fn load(path: PathBuf) -> Self {
        let state = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|_| HashMap::new()),
                Err(_) => HashMap::new(),
            }
        } else {
            HashMap::new()
        };
        Self {
            state: Mutex::new(state),
            path,
        }
    }

    /// 持久化当前内存状态到文件。
    fn persist(&self) -> Result<(), AgentError> {
        let state = self.state.lock().unwrap();
        let json = serde_json::to_string_pretty(&*state)
            .map_err(|e| AgentError::Context(e.to_string()))?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AgentError::Context(format!("create artifacts dir: {e}")))?;
        }
        fs::write(&self.path, json)
            .map_err(|e| AgentError::Context(format!("write scratchpad.json: {e}")))?;
        Ok(())
    }
}

/// `scratchpad` 工具。
pub struct ScratchpadTool {
    store: ScratchpadStore,
}

impl ScratchpadTool {
    /// 构造 scratchpad 工具，使用给定 workspace 下的 `artifacts/scratchpad.json`。
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(workspace: PathBuf) -> Self {
        let path = workspace.join("artifacts").join("scratchpad.json");
        Self {
            store: ScratchpadStore::load(path),
        }
    }

    /// 用已有 store 构造（供测试注入）。
    #[allow(dead_code)]
    pub(crate) fn with_store(store: ScratchpadStore) -> Self {
        Self { store }
    }

    /// 生成 scratchpad 摘要，供 compaction 注入。
    ///
    /// 读取 `{workspace}/artifacts/scratchpad.json`，返回 `"key1: 200B, key2: 150B"`
    /// 格式的摘要。若文件不存在或为空，返回 `None`。
    #[must_use]
    pub fn summary(workspace: &std::path::Path) -> Option<String> {
        let path = workspace.join("artifacts").join("scratchpad.json");
        if !path.exists() {
            return None;
        }
        let contents = std::fs::read_to_string(&path).ok()?;
        let state: std::collections::HashMap<String, ScratchpadEntry> =
            serde_json::from_str(&contents).ok()?;
        if state.is_empty() {
            return None;
        }
        let summary = state
            .iter()
            .map(|(k, v)| format!("{}: {}B", k, v.value.len()))
            .collect::<Vec<_>>()
            .join(", ");
        Some(summary)
    }
}

impl Tool for ScratchpadTool {
    fn name(&self) -> &'static str {
        "scratchpad"
    }

    fn description(&self) -> &'static str {
        "Persistent key-value store. Survives across agent turns."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["set", "get", "append", "list", "clear"],
                        "description": "Action to perform."
                    },
                    "name": {
                        "type": "string",
                        "description": "Key name. Required for set/get/append."
                    },
                    "value": {
                        "type": "string",
                        "description": "Value content. Required for set/append."
                    },
                    "idempotent": {
                        "type": "boolean",
                        "description": "If true and key exists with same value, return 'unchanged'."
                    }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
        }
    }

    fn execute(&self, input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        let v = input.arguments.as_value();
        let action = v
            .get("action")
            .and_then(|a| a.as_str())
            .ok_or_else(|| AgentError::InvalidArguments("missing `action` field".into()))?;

        let name = v.get("name").and_then(|n| n.as_str());
        let value = v.get("value").and_then(|v| v.as_str());
        let idempotent = v
            .get("idempotent")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        match action {
            "set" => {
                let name =
                    name.ok_or_else(|| AgentError::InvalidArguments("missing `name`".into()))?;
                let value =
                    value.ok_or_else(|| AgentError::InvalidArguments("missing `value`".into()))?;
                let mut state = self.store.state.lock().unwrap();
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d: std::time::Duration| d.as_secs() as i64);
                if idempotent {
                    if let Some(existing) = state.get(name) {
                        if existing.value == value {
                            return Ok(ToolOutput::ok("unchanged"));
                        }
                    }
                }
                state.insert(
                    name.to_string(),
                    ScratchpadEntry {
                        value: value.to_string(),
                        updated_at: now,
                    },
                );
                drop(state);
                self.store.persist()?;
                Ok(ToolOutput::ok(format!("set `{name}`")))
            }
            "get" => {
                let name =
                    name.ok_or_else(|| AgentError::InvalidArguments("missing `name`".into()))?;
                let state = self.store.state.lock().unwrap();
                match state.get(name) {
                    Some(entry) => Ok(ToolOutput::ok(&entry.value)),
                    None => Err(AgentError::Context(format!("key not found: `{name}`"))),
                }
            }
            "append" => {
                let name =
                    name.ok_or_else(|| AgentError::InvalidArguments("missing `name`".into()))?;
                let value =
                    value.ok_or_else(|| AgentError::InvalidArguments("missing `value`".into()))?;
                let mut state = self.store.state.lock().unwrap();
                match state.get_mut(name) {
                    Some(entry) => {
                        entry.value.push('\n');
                        entry.value.push_str(value);
                        entry.updated_at = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map_or(0, |d: std::time::Duration| d.as_secs() as i64);
                        drop(state);
                        self.store.persist()?;
                        Ok(ToolOutput::ok(format!("appended to `{name}`")))
                    }
                    None => Err(AgentError::Context(format!("key not found: `{name}`"))),
                }
            }
            "list" => {
                let state = self.store.state.lock().unwrap();
                let entries: Vec<serde_json::Value> = state
                    .iter()
                    .map(|(k, e)| {
                        serde_json::json!({
                            "name": k,
                            "bytes": e.value.len(),
                            "updated_at": e.updated_at
                        })
                    })
                    .collect();
                Ok(ToolOutput::ok(
                    serde_json::to_string_pretty(&entries).unwrap(),
                ))
            }
            "clear" => {
                let count = {
                    let mut state = self.store.state.lock().unwrap();
                    let count = state.len();
                    state.clear();
                    count
                };
                self.store.persist()?;
                Ok(ToolOutput::ok(format!("{count} entries cleared")))
            }
            other => Err(AgentError::InvalidArguments(format!(
                "unknown action: `{other}`"
            ))),
        }
    }
}
