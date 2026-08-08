//! Server-Sent Events (SSE) 解析器。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.2（v0.2）。
//!
//! - 输入：`&[u8]` 字节流（可能跨多条事件的边界）。
//! - 输出：每个完整事件 `SseEvent`。
//! - 不依赖第三方 crate（保持 KISS），单遍状态机。
//!
//! v1 支持的事件字段：
//! - `data:`：UTF-8 解码后累积到 `data`；多个 `data:` 行用 `\n` 拼接。
//! - `event:`：覆盖事件类型（默认 `None`，可省略）。
//! - `id:`：记录 `id`（默认 `None`）。
//! - 空行：dispatch 当前累积的事件。
//! - 以 `:` 开头的行为注释，忽略。
//!
//! 不实现（v1 故意不做）：retry、event-stream 自动重连、UTF-8 BOM 处理。

use thiserror::Error;

/// 单条解析出的 SSE 事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// 事件类型（来自 `event:` 字段；缺省时为 `None`，调用方按 `"message"` 处理）。
    pub event: Option<String>,
    /// `data:` 字段累积内容（多个 `data:` 行用 `\n` 拼接）。
    pub data: String,
    /// `id:` 字段。
    pub id: Option<String>,
}

/// 解析错误。
#[derive(Debug, Error)]
pub enum SseError {
    /// 字节流不是合法 UTF-8。
    #[error("non-utf8 byte in SSE stream at byte offset {0}")]
    InvalidUtf8(usize),
}

/// SSE 解析器状态机。
#[derive(Debug, Default)]
pub struct SseParser {
    /// 当前事件累积区。
    buf: SseBuffer,
}

#[derive(Debug, Default)]
struct SseBuffer {
    event: Option<String>,
    data: Vec<String>,
    id: Option<String>,
}

impl SseBuffer {
    fn clear(&mut self) {
        self.event = None;
        self.data.clear();
        self.id = None;
    }

    fn to_event(&self) -> Option<SseEvent> {
        if self.event.is_none() && self.data.is_empty() && self.id.is_none() {
            return None;
        }
        Some(SseEvent {
            event: self.event.clone(),
            data: self.data.join("\n"),
            id: self.id.clone(),
        })
    }
}

impl SseParser {
    /// 构造空解析器。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂入一段字节，返回已解析出的完整事件列表。
    ///
    /// 未完整的事件（缺少终止空行）会保留在内部缓冲区，下次 `feed` 续接。
    ///
    /// # Errors
    ///
    /// - [`SseError::InvalidUtf8`]：字节流包含非法 UTF-8 序列。
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<SseEvent>, SseError> {
        let text =
            std::str::from_utf8(bytes).map_err(|e| SseError::InvalidUtf8(e.valid_up_to()))?;

        let mut out = Vec::new();
        for raw_line in text.split('\n') {
            // SSE 行以 \r\n 结尾时，split('\n') 会留下 \r
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

            if line.is_empty() {
                // 空行 = dispatch
                if let Some(ev) = self.buf.to_event() {
                    out.push(ev);
                }
                self.buf.clear();
                continue;
            }

            if let Some(rest) = line.strip_prefix(':') {
                // 注释行，忽略
                let _ = rest;
                continue;
            }

            // 解析 field: value
            let (field, value) = match line.find(':') {
                Some(idx) => {
                    let value = line[idx + 1..].trim_start().to_string();
                    (&line[..idx], value)
                }
                None => (line, String::new()),
            };

            match field {
                "event" => self.buf.event = Some(value),
                "data" => self.buf.data.push(value),
                "id" => self.buf.id = Some(value),
                _ => {
                    // 未知字段忽略（retry 等 v1 不处理）
                }
            }
        }

        Ok(out)
    }

    /// 标记流结束；返回缓冲区中尚未 dispatch 的事件（若有）。
    #[must_use]
    pub fn finish(&mut self) -> Option<SseEvent> {
        let ev = self.buf.to_event();
        self.buf.clear();
        ev
    }
}
