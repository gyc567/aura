//! 纯领域类型：任务请求、消息、决策、工具调用。
//!
//! 这些类型不执行任何 IO，也不依赖具体模型或终端实现。
//! 它们的不变式通过构造函数与 `validate` 方法集中维护。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::AgentError;

/// 用户提交的一次任务。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRequest {
    /// 自然语言指令，必须非空。
    pub instruction: String,
    /// 工作区绝对路径。
    pub workspace: PathBuf,
    /// 最大循环轮次，必须 > 0。
    pub max_turns: u32,
    /// 最大子代理递归深度（v1.2 新增）。0 = 不允许再嵌套 subagent。
    pub max_depth: u32,
}

impl TaskRequest {
    /// 构造任务并校验不变式。
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidRequest`]：指令为空、`max_turns` 为 0、
    ///   或 `workspace` 不是绝对路径。
    pub fn new(
        instruction: impl Into<String>,
        workspace: PathBuf,
        max_turns: u32,
    ) -> Result<Self, AgentError> {
        Self::new_with_depth(instruction, workspace, max_turns, 2)
    }

    /// 构造任务并指定递归深度。
    ///
    /// # Errors
    ///
    /// 同 [`Self::new`]。
    pub fn new_with_depth(
        instruction: impl Into<String>,
        workspace: PathBuf,
        max_turns: u32,
        max_depth: u32,
    ) -> Result<Self, AgentError> {
        let request = Self {
            instruction: instruction.into(),
            workspace,
            max_turns,
            max_depth,
        };
        request.validate()?;
        Ok(request)
    }

    /// 校验当前实例的不变式。
    ///
    /// # Errors
    ///
    /// 同 [`Self::new`]。
    pub fn validate(&self) -> Result<(), AgentError> {
        if self.instruction.trim().is_empty() {
            return Err(AgentError::InvalidRequest(
                "instruction must not be empty".into(),
            ));
        }
        if self.max_turns == 0 {
            return Err(AgentError::InvalidRequest(
                "max_turns must be greater than zero".into(),
            ));
        }
        if self.max_depth == 0 {
            return Err(AgentError::InvalidRequest(
                "max_depth must be greater than zero".into(),
            ));
        }
        if !self.workspace.is_absolute() {
            return Err(AgentError::InvalidRequest(
                "workspace must be an absolute path".into(),
            ));
        }
        Ok(())
    }
}

/// 对话历史中的单条消息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    /// 系统提示词。
    System {
        /// 提示内容。
        content: String,
    },
    /// 用户消息或任务指令。
    User {
        /// 消息内容。
        content: String,
    },
    /// 助手消息，通常是结构化 `Decision` 的可读投影。
    Assistant {
        /// 消息内容。
        content: String,
        /// 本消息携带的工具调用（OpenAI 兼容协议要求 tool result 前有声明 tool id 的 assistant 消息）。
        #[serde(default)]
        tool_calls: Vec<ToolCall>,
    },
    /// 工具结果反馈。
    Tool {
        /// 关联的 `ToolCall.id`。
        call_id: String,
        /// 工具输出文本。
        output: String,
        /// 工具是否成功。
        success: bool,
    },
}

/// 模型在每轮循环中给出的决策。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    /// 调用一个工具。
    Call(ToolCall),
    /// 向用户请求澄清。
    Ask {
        /// 需要回答的问题。
        question: String,
    },
    /// 任务完成。
    Done {
        /// 完成摘要，将出现在测试报告中。
        summary: String,
    },
    /// 任务失败，停止循环。
    Fail {
        /// 失败原因。
        reason: String,
    },
}

impl Decision {
    /// 校验决策是否语义合法。
    ///
    /// # Errors
    ///
    /// - [`AgentError::UnparseableDecision`]：`Ask` 或 `Done` 的字符串为空。
    pub fn validate(&self) -> Result<(), AgentError> {
        match self {
            Self::Call(call) => call.validate(),
            Self::Ask { question } => {
                if question.trim().is_empty() {
                    Err(AgentError::UnparseableDecision(
                        "ask question must not be empty".into(),
                    ))
                } else {
                    Ok(())
                }
            }
            Self::Done { summary } => {
                if summary.trim().is_empty() {
                    Err(AgentError::UnparseableDecision(
                        "done summary must not be empty".into(),
                    ))
                } else {
                    Ok(())
                }
            }
            Self::Fail { reason } => {
                if reason.trim().is_empty() {
                    Err(AgentError::UnparseableDecision(
                        "fail reason must not be empty".into(),
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// 一次工具调用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// 调用唯一 id，用于回填到 `Message::Tool`。
    pub id: String,
    /// 工具注册名。
    pub name: String,
    /// 工具参数，序列化为 JSON 对象。
    pub arguments: ToolArgument,
}

impl ToolCall {
    /// 构造并校验 `ToolCall`。
    ///
    /// # Errors
    ///
    /// - [`AgentError::UnparseableDecision`]：`id` 或 `name` 为空。
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: ToolArgument,
    ) -> Result<Self, AgentError> {
        let call = Self {
            id: id.into(),
            name: name.into(),
            arguments,
        };
        call.validate()?;
        Ok(call)
    }

    /// 校验当前实例。
    ///
    /// # Errors
    ///
    /// 同 [`Self::new`]。
    pub fn validate(&self) -> Result<(), AgentError> {
        if self.id.trim().is_empty() {
            return Err(AgentError::UnparseableDecision(
                "tool call id must not be empty".into(),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(AgentError::UnparseableDecision(
                "tool call name must not be empty".into(),
            ));
        }
        Ok(())
    }
}

/// 工具参数包装：限制为结构化 JSON 值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolArgument(serde_json::Value);

impl ToolArgument {
    /// 用 JSON 值构造。
    #[must_use]
    pub fn new(value: serde_json::Value) -> Self {
        Self(value)
    }

    /// 构造空对象 `{}`。
    #[must_use]
    pub fn empty() -> Self {
        Self(serde_json::Value::Object(serde_json::Map::new()))
    }

    /// 以 `&serde_json::Value` 形式读取。
    #[must_use]
    pub fn as_value(&self) -> &serde_json::Value {
        &self.0
    }

    /// 转为拥有的 JSON 值。
    #[must_use]
    pub fn into_value(self) -> serde_json::Value {
        self.0
    }
}

impl Default for ToolArgument {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<serde_json::Value> for ToolArgument {
    fn from(value: serde_json::Value) -> Self {
        Self(value)
    }
}

/// 子代理唯一标识（v1.2 新增：RLM 式子代理）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChildId(pub String);

impl ChildId {
    /// 生成一个唯一的子代理 ID。
    #[must_use]
    pub fn generate() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Self(format!("child_{}", now.as_nanos()))
    }
}

impl std::fmt::Display for ChildId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 子代理运行状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildStatus {
    /// 子代理正在后台运行。
    Running,
    /// 子代理已完成。
    Completed,
    /// 子代理失败。
    Failed,
}

/// 子代理句柄：父作用域中 `ChildRegistry` 的一项。
#[derive(Debug)]
pub struct ChildHandle {
    /// 子代理 ID。
    pub child_id: ChildId,
    /// 子代理名称（可选）。
    pub name: Option<String>,
    /// 子代理独立会话目录。
    pub session_dir: PathBuf,
    /// 当前状态。
    pub status: ChildStatus,
    /// 完成结果摘要（完成后可用）。
    pub result: Option<String>,
    /// 发送给子代理的待处理消息。
    pub inbox: Vec<AgentMessage>,
}

/// 父→子代理的定向消息（v1.2 新增）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// 接收方子代理 ID。
    pub to: ChildId,
    /// 发送方标识（父代理或子代理 ID）。
    pub from: String,
    /// 消息内容。
    pub content: String,
}
