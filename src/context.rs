//! 上下文收集与截断。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.4 与 §2.5。
//!
//! - `collect_workspace_files`：扫描 workspace 内允许的文件（拒绝敏感路径）。
//! - `is_sensitive`：检测是否在默认 deny list 中（`.env`、`*.pem`、`*.key`、`*.ssh` 等）。
//! - `truncate_messages`：按优先级截断 message 列表，保持总字节数 ≤ `max_bytes`。
//!
//! v1 不实现按 git diff 自动选择文件；调用方按 `TaskRequest.instruction` 提示的路径传入。

use std::path::{Path, PathBuf};

use crate::domain::Message;
use crate::error::AgentError;

/// 上下文条目：路径 + 内容 + 优先级（截断时使用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextFile {
    /// 文件路径（绝对，相对 workspace）。
    pub path: PathBuf,
    /// 文件内容。
    pub content: String,
    /// 截断优先级（值越小越优先保留）。
    pub priority: ContextPriority,
}

/// 截断优先级。值越小越重要；同优先级先出现的优先保留。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContextPriority {
    /// 系统提醒与 instruction（永不截断）。
    System = 0,
    /// 用户首条指令。
    UserInstruction = 1,
    /// 最近 3-5 条工具结果。
    RecentTool = 2,
    /// 早期对话（优先截断）。
    Early = 3,
}

/// 敏感路径判定（v1 保守黑名单）。
///
/// # Returns
///
/// `true` 当路径匹配任一敏感规则。规则：
/// - 隐藏文件 / 目录（`.git/`、`.env*`、`*.ssh/` 等）以 `.` 开头且文件名命中黑名单。
/// - 文件名以 `.pem` / `.key` / `.pfx` 结尾。
/// - 路径段包含 `secrets/` / `credentials/` / `.ssh/`。
#[must_use]
pub fn is_sensitive(path: &Path) -> bool {
    for component in path.components() {
        let s = component.as_os_str().to_string_lossy();
        if s.starts_with('.') && is_sensitive_dotfile(&s) {
            return true;
        }
        if s.ends_with(".pem") || s.ends_with(".key") || s.ends_with(".pfx") {
            return true;
        }
        if s == ".ssh" || s == "secrets" || s == "credentials" {
            return true;
        }
    }
    false
}

fn is_sensitive_dotfile(name: &str) -> bool {
    matches!(
        name,
        ".env" | ".envrc" | ".git" | ".ssh" | ".aws" | ".npmrc"
    )
}

/// 扫描 workspace 收集允许读入上下文的文件。
///
/// # Errors
///
/// - [`AgentError::Context`]：扫描 IO 错误。
/// - [`AgentError::PathPolicy`]：workspace 路径不在允许范围。
pub fn collect_workspace_files(
    workspace: &Path,
    include: &[PathBuf],
) -> Result<Vec<ContextFile>, AgentError> {
    let mut files = Vec::new();
    for rel in include {
        let abs = if rel.is_absolute() {
            rel.clone()
        } else {
            workspace.join(rel)
        };
        if is_sensitive(&abs) {
            return Err(AgentError::PathPolicy(format!(
                "sensitive path refused: {}",
                abs.display()
            )));
        }
        let content = std::fs::read_to_string(&abs)
            .map_err(|e| AgentError::Context(format!("read {} failed: {e}", abs.display())))?;
        files.push(ContextFile {
            path: abs,
            content,
            priority: ContextPriority::RecentTool,
        });
    }
    Ok(files)
}

/// 截断 message 列表。`max_bytes` 是总字节上限；按优先级分组保留。
///
/// 策略（v1）：
/// 1. 始终保留 `System` 与 `UserInstruction` 优先级条目。
/// 2. 优先保留最近的 `RecentTool` 条目（最多 5 条）。
/// 3. 按 `Early` 优先级的反向顺序丢弃（先丢最新的 Early）。
/// 4. 截断单位是整条 `Message`；不拆半条。
///
/// # Errors
///
/// 当前实现不返回错误；未来若加入语义校验会从此处返回。
pub fn truncate_messages(
    messages: Vec<Message>,
    max_bytes: u64,
) -> Result<TruncationResult, AgentError> {
    let original_bytes = messages.iter().map(message_bytes).sum::<u64>();

    if original_bytes <= max_bytes {
        return Ok(TruncationResult {
            messages,
            original_bytes,
            truncated_bytes: original_bytes,
            dropped: 0,
        });
    }

    // 按消息类型估算优先级。
    // 简化：System reminder 优先；User 首条 → UserInstruction；Tool → RecentTool；其它 → Early。
    let annotated: Vec<(usize, MessageKind, u64)> = messages
        .iter()
        .enumerate()
        .map(|(i, m)| (i, classify(m), message_bytes(m)))
        .collect();

    let mut keep: Vec<bool> = vec![true; messages.len()];

    // 1. 标记 System / UserInstruction 为必保留。
    for (i, kind, _) in &annotated {
        if matches!(kind, MessageKind::System | MessageKind::UserInstruction) {
            keep[*i] = true;
        }
    }

    // 2. 收集 RecentTool，按"最近优先"排序。
    let mut recent_tools: Vec<usize> = annotated
        .iter()
        .filter(|(_, k, _)| matches!(k, MessageKind::RecentTool))
        .map(|(i, _, _)| *i)
        .collect();
    recent_tools.reverse(); // 最近的在最前
    let max_recent: usize = 5;
    for &i in recent_tools.iter().skip(max_recent) {
        keep[i] = false;
    }
    for &i in recent_tools.iter().take(max_recent) {
        keep[i] = true;
    }

    // 3. 累计保留字节数；若超 max_bytes，从 Early 末尾开始丢。
    let mut kept_bytes: u64 = annotated
        .iter()
        .filter(|(i, _, _)| keep[*i])
        .map(|(_, _, b)| *b)
        .sum();

    let mut early_indices: Vec<usize> = annotated
        .iter()
        .filter(|(_, k, _)| matches!(k, MessageKind::Early))
        .map(|(i, _, _)| *i)
        .collect();
    early_indices.sort_unstable(); // 按时间顺序，最早在前
    early_indices.reverse(); // 最新 Early 在前，先丢它

    let mut dropped: usize = 0;
    for i in early_indices {
        if kept_bytes <= max_bytes {
            break;
        }
        if keep[i] {
            kept_bytes = kept_bytes.saturating_sub(annotated[i].2);
            keep[i] = false;
            dropped += 1;
        }
    }

    let truncated: Vec<Message> = messages
        .into_iter()
        .zip(keep.iter())
        .filter_map(|(m, &k)| if k { Some(m) } else { None })
        .collect();
    let truncated_bytes = truncated.iter().map(message_bytes).sum();

    Ok(TruncationResult {
        messages: truncated,
        original_bytes,
        truncated_bytes,
        dropped,
    })
}

/// 消息分类（截断优先级）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageKind {
    /// 系统提醒，永不截断。
    System,
    /// 用户首条指令，永不截断。
    UserInstruction,
    /// 工具结果，保留最近 5 条。
    RecentTool,
    /// 早期对话，可截断。
    Early,
}

fn classify(m: &Message) -> MessageKind {
    match m {
        Message::System { .. } => MessageKind::System,
        Message::User { .. } => MessageKind::UserInstruction,
        Message::Tool { .. } => MessageKind::RecentTool,
        Message::Assistant { .. } => MessageKind::Early,
    }
}

fn message_bytes(m: &Message) -> u64 {
    let s: String = match m {
        Message::System { content }
        | Message::User { content }
        | Message::Assistant { content, .. } => content.clone(),
        Message::Tool { output, .. } => output.clone(),
    };
    s.len() as u64
}

/// 截断结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncationResult {
    /// 截断后保留的 messages。
    pub messages: Vec<Message>,
    /// 截断前总字节数。
    pub original_bytes: u64,
    /// 截断后总字节数。
    pub truncated_bytes: u64,
    /// 丢弃的 message 数量。
    pub dropped: usize,
}
