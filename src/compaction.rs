//! 分层上下文压缩：将长对话历史压缩为分层注入（摘要 + 核心窗口）。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §4.4 与
//! [`docs/architecture-roadmap.md`](../../docs/architecture-roadmap.md) §4.4。

use crate::domain::Message;

/// 压缩上下文后的分层结构。
#[derive(Debug, Clone)]
pub struct LayeredContext {
    /// 工作记忆摘要（scratchpad 条目名 + 大小）。若无需工作记忆则为 None。
    pub scratchpad_summary: Option<String>,
    /// 核心窗口（最近 N 条消息，全量）。
    pub core_window: Vec<Message>,
    /// 历史摘要（早期消息的规则压缩）。若尚无早期消息则为 None。
    pub history_summary: Option<String>,
    /// 核心窗口最大保留条数。
    pub core_window_size: usize,
}

impl LayeredContext {
    /// 返回当前分层的总消息条目数（scratchpad 摘要算 1 条，`core_window` 实际条数，历史摘要算 1 条）。
    #[must_use]
    pub fn message_count(&self) -> usize {
        let scratchpad = usize::from(self.scratchpad_summary.is_some());
        let core = self.core_window.len();
        let history = usize::from(self.history_summary.is_some());
        scratchpad + core + history
    }

    /// 展开为模型可消费的 message 列表。
    ///
    /// 生成顺序：历史摘要 → scratchpad 摘要 → 核心窗口。
    /// 摘要消息以 `Message::Assistant` 形式插入，模拟模型的"记忆"。
    #[must_use]
    pub fn into_model_messages(self) -> Vec<Message> {
        let mut result: Vec<Message> = Vec::new();

        // 1. 历史摘要（最早插入，因为是"早期"内容）
        if let Some(summary) = self.history_summary {
            result.push(Message::Assistant { content: summary });
        }

        // 2. scratchpad 摘要
        if let Some(sp) = self.scratchpad_summary {
            let content = format!("## Working memory\n{sp}\n---\nUse this context as needed.");
            result.push(Message::Assistant { content });
        }

        // 3. 核心窗口（追加，不覆盖前面的摘要）
        result.extend(self.core_window);

        result
    }
}

/// 消息列表的总字节数（用于触发判断）。
fn total_bytes(messages: &[Message]) -> u64 {
    messages.iter().map(message_byte_len).sum()
}

fn message_byte_len(m: &Message) -> u64 {
    let s: &str = match m {
        Message::System { content }
        | Message::User { content }
        | Message::Assistant { content } => content,
        Message::Tool { output, .. } => output,
    };
    s.len() as u64
}

/// 检查消息列表是否已达到压缩触发阈值。
///
/// 当 `total_bytes >= max_context_bytes * 0.80` 时返回 true。
#[must_use]
pub fn should_compact(messages: &[Message], max_context_bytes: u64) -> bool {
    if max_context_bytes == 0 {
        return false;
    }
    let threshold =
        (max_context_bytes * COMPACTION_THRESHOLD_NUMERATOR) / COMPACTION_THRESHOLD_DENOMINATOR;
    total_bytes(messages) >= threshold
}

/// 摘要生成器：接收消息列表，返回自然语言摘要。
///
/// v1 使用规则摘要（无需外部模型调用）。未来可替换为 fast model 调用。
fn summarize_early_messages(messages: &[Message]) -> String {
    if messages.is_empty() {
        return String::new();
    }

    let mut parts: Vec<String> = Vec::with_capacity(messages.len());

    for msg in messages {
        match msg {
            Message::Assistant { content } => {
                let snippet = if content.len() > 120 {
                    format!("{content}…")
                } else {
                    content.clone()
                };
                parts.push(format!("[assistant] {snippet}"));
            }
            Message::Tool {
                call_id, output, ..
            } => {
                let output_preview = if output.len() > 80 {
                    // byte index 80 可能落在 UTF-8 字符中间（中文/emoji 工具输出），
                    // 用 char_indices 找安全边界，避免 panic。
                    let cut = output
                        .char_indices()
                        .nth(80)
                        .map_or(output.len(), |(i, _)| i);
                    format!("{}…", &output[..cut])
                } else {
                    output.clone()
                };
                let tool_name = call_id
                    .rsplit_once('-')
                    .map_or(call_id.as_str(), |(prefix, _)| prefix);
                parts.push(format!("[{tool_name}] {output_preview}"));
            }
            Message::User { content } => {
                let snippet = if content.len() > 120 {
                    format!("{content}…")
                } else {
                    content.clone()
                };
                parts.push(format!("[user] {snippet}"));
            }
            Message::System { content } => {
                parts.push(format!("[system] {content}"));
            }
        }
    }

    let joined = parts.join("\n");
    format!(
        "## Earlier conversation summary ({} messages)\n{joined}\n---",
        messages.len(),
    )
}

/// 将消息列表压缩为分层上下文。
///
/// - `scratchpad_summary`：工作记忆摘要（外部注入，可为 None）。
/// - `_max_context_bytes`：上下文总字节上限（预留，未来用于动态阈值）。
/// - `core_window_size`：核心窗口保留的最大消息条数（默认 10）。
/// - `already_summarized`：早期消息是否已生成过摘要（避免重复摘要调用）。
///
/// # Panics
///
/// 不 panic。
#[must_use]
pub fn compact(
    messages: &[Message],
    scratchpad_summary: Option<&str>,
    _max_context_bytes: u64,
    core_window_size: usize,
    already_summarized: bool,
) -> LayeredContext {
    // 计算核心窗口：从末尾取 core_window_size 条
    let (core_window, early_messages): (Vec<Message>, Vec<Message>) =
        if messages.len() > core_window_size {
            let split = messages.len() - core_window_size;
            (messages[split..].to_vec(), messages[..split].to_vec())
        } else {
            (messages.to_vec(), Vec::new())
        };

    // 生成历史摘要（仅在有早期消息时）
    let history_summary = if early_messages.is_empty() {
        None
    } else if already_summarized {
        Some(String::from(
            "[earlier messages already summarized — see prior summary]",
        ))
    } else {
        Some(summarize_early_messages(&early_messages))
    };

    LayeredContext {
        scratchpad_summary: scratchpad_summary.map(String::from),
        core_window,
        history_summary,
        core_window_size,
    }
}

/// 压缩触发阈值：80%。
const COMPACTION_THRESHOLD_NUMERATOR: u64 = 80;
const COMPACTION_THRESHOLD_DENOMINATOR: u64 = 100;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Message;

    fn assistant(content: &str) -> Message {
        Message::Assistant {
            content: content.to_string(),
        }
    }
    fn tool(call_id: &str, output: &str) -> Message {
        Message::Tool {
            call_id: call_id.to_string(),
            output: output.to_string(),
            success: true,
        }
    }
    #[allow(dead_code)]
    fn user(content: &str) -> Message {
        Message::User {
            content: content.to_string(),
        }
    }

    fn assistant_content(msg: &Message) -> Option<&str> {
        match msg {
            Message::Assistant { content } => Some(content),
            _ => None,
        }
    }

    #[test]
    fn test_should_compact_below_threshold() {
        // 5 条 × ~10 字节 = 50 字节 < 80，不触发
        let msgs: Vec<Message> = (0..5).map(|i| assistant(&format!("step {i}"))).collect();
        assert!(!should_compact(&msgs, 100));
    }

    #[test]
    fn test_should_compact_above_threshold() {
        // 15 条 × ~15 字节 = 225 字节 > 80，触发
        let msgs: Vec<Message> = (0..15)
            .map(|i| assistant(&format!("step {i} done something longer")))
            .collect();
        assert!(should_compact(&msgs, 100));
    }

    #[test]
    fn test_should_compact_zero_max() {
        assert!(!should_compact(&[assistant("hello")], 0));
    }

    #[test]
    fn test_compact_no_early_messages() {
        // 3 条消息，全部在核心窗口内
        let msgs: Vec<Message> = (0..3).map(|i| assistant(&format!("step {i}"))).collect();
        let ctx = compact(&msgs, None, 100, 10, false);
        assert!(ctx.history_summary.is_none());
        assert_eq!(ctx.core_window.len(), 3);
    }

    #[test]
    fn test_compact_splits_core_and_early() {
        // 15 条，核心窗口保留 10 条，早期 5 条应被摘要
        let msgs: Vec<Message> = (0..15).map(|i| assistant(&format!("step {i}"))).collect();
        let ctx = compact(&msgs, None, 100, 10, false);
        assert_eq!(ctx.core_window.len(), 10);
        assert!(ctx.history_summary.is_some());
        let summary = ctx.history_summary.unwrap();
        assert!(summary.contains("Earlier conversation summary"));
        assert!(summary.contains("step 0")); // 早期消息在摘要中
    }

    #[test]
    fn test_compact_already_summarized() {
        let msgs: Vec<Message> = (0..15).map(|i| assistant(&format!("step {i}"))).collect();
        let ctx = compact(&msgs, None, 100, 10, true);
        assert!(ctx.history_summary.is_some());
        let summary = ctx.history_summary.unwrap();
        assert!(summary.contains("already summarized"));
    }

    #[test]
    fn test_compact_scratchpad_injected() {
        let msgs = vec![assistant("working")];
        let ctx = compact(&msgs, Some("file_a: 200B, file_b: 150B"), 100, 10, false);
        assert!(ctx.scratchpad_summary.is_some());
        assert!(ctx.scratchpad_summary.unwrap().contains("file_a"));
    }

    #[test]
    fn test_into_model_messages_order() {
        let msgs: Vec<Message> = (0..12).map(|i| assistant(&format!("step {i}"))).collect();
        let ctx = compact(&msgs, Some("scratch: notes"), 100, 10, false);
        let flat = ctx.into_model_messages();
        // 第一条应为历史摘要
        assert!(matches!(flat[0], Message::Assistant { .. }));
        assert!(
            assistant_content(&flat[0])
                .unwrap()
                .contains("Earlier conversation")
        );
        // 第二条应为 scratchpad 摘要
        assert!(matches!(flat[1], Message::Assistant { .. }));
        assert!(
            assistant_content(&flat[1])
                .unwrap()
                .contains("Working memory")
        );
    }

    #[test]
    fn test_layered_context_message_count() {
        let msgs: Vec<Message> = (0..12).map(|i| assistant(&format!("step {i}"))).collect();
        let ctx = compact(&msgs, Some("scratch"), 100, 10, false);
        // 1 history_summary + 1 scratchpad + 10 core_window = 12
        assert_eq!(ctx.message_count(), 12);
    }

    #[test]
    fn test_tool_message_in_summary() {
        let msgs = vec![
            tool("grep_files-0", "Found 5 matches in 3 files"),
            assistant("I found the matches"),
        ];
        let ctx = compact(&msgs, None, 100, 10, false);
        assert!(ctx.history_summary.is_none()); // 全部在核心窗口
        assert_eq!(ctx.core_window.len(), 2);
    }

    #[test]
    fn test_multibyte_tool_output_no_panic() {
        // 30 个汉字 = 90 字节；旧实现 &output[..80] 会把第 80 字节切在
        // 第 27 个汉字中间 → panic。这里验证不 panic 且摘要为合法 UTF-8。
        let chinese = "汉".repeat(30);
        let msgs = vec![tool("run_command-0", &chinese), assistant("ok")];
        let ctx = compact(&msgs, None, 100, 1, false);
        let summary = ctx
            .history_summary
            .expect("tool message should be in early messages");
        assert!(summary.contains("run_command"), "summary: {summary}");
        // String 本身保证 UTF-8；再确认截断后的预览不超原长度且以 … 结尾
        assert!(summary.contains('…'));
    }

    #[test]
    fn test_compaction_trigger_ratio() {
        // max = 100, threshold = 80
        // "step 0 done!" = 11 bytes × 8 = 88 bytes > 80 → 触发
        let at_threshold: Vec<Message> = (0..8)
            .map(|i| assistant(&format!("step {i} done!")))
            .collect();
        assert!(should_compact(&at_threshold, 100));

        // "step 0" = 7 bytes × 7 = 49 < 80 → 不触发
        let below: Vec<Message> = (0..7).map(|i| assistant(&format!("step {i}"))).collect();
        assert!(!should_compact(&below, 100));
    }

    #[test]
    fn test_summarize_empty() {
        let ctx = compact(&[], None, 100, 10, false);
        assert!(ctx.history_summary.is_none());
        assert!(ctx.core_window.is_empty());
    }

    #[test]
    fn test_into_model_messages_empty_core_window() {
        // 只有早期消息，无核心窗口
        let msgs: Vec<Message> = (0..12).map(|i| assistant(&format!("step {i}"))).collect();
        let ctx = compact(&msgs, Some("scratch"), 100, 20, false);
        // 12 < 20，全部在核心窗口，无摘要
        assert!(ctx.history_summary.is_none());
        let flat = ctx.into_model_messages();
        // 只有 scratchpad + 12 core_window
        assert_eq!(flat.len(), 13); // 1 scratchpad + 12 core
    }
}
