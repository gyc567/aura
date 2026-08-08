//! 运行报告输出格式。
//!
//! - `--print`（默认）：人类可读文本，包含 Agent run 状态、TODO 状态、变更摘要、验证。
//! - `--json`：稳定 JSON（schema `aura.report.v1`），便于脚本消费。

use std::fmt::Write as _;

use serde::Serialize;

use crate::agent::RunReport;

/// JSON 报告（schema `aura.report.v1`）。
#[derive(Debug, Clone, Serialize)]
pub struct JsonReport {
    /// schema 版本。
    pub schema: String,
    /// 任务指令。
    pub instruction: String,
    /// 工作区路径。
    pub workspace: String,
    /// 退出状态：ok / failed / aborted。
    pub status: String,
    /// 实际使用轮次。
    pub used_turns: u32,
    /// 摘要（完成 / 失败原因 / Ask 问题 / 中断）。
    pub summary: String,
    /// TODO 最终状态（来自调用历史回放；v1 简化：始终为空数组）。
    pub todo_final: Vec<()>,
}

impl JsonReport {
    /// 从 `RunReport` 构造。
    #[must_use]
    pub fn from_report(instruction: &str, workspace: &Path, report: &RunReport) -> Self {
        let (status, summary) = match &report.stop_reason {
            crate::agent::StopReasonPayload::Completed { summary } => {
                ("ok".to_string(), summary.clone())
            }
            crate::agent::StopReasonPayload::ModelFailed { reason } => {
                ("failed".to_string(), format!("model failed: {reason}"))
            }
            crate::agent::StopReasonPayload::ModelAsked { question } => {
                ("failed".to_string(), format!("model asked: {question}"))
            }
            crate::agent::StopReasonPayload::BudgetExhausted { used } => (
                "failed".to_string(),
                format!("budget exhausted at turn {used}"),
            ),
            crate::agent::StopReasonPayload::ToolFailed { message } => {
                ("failed".to_string(), format!("tool failed: {message}"))
            }
            crate::agent::StopReasonPayload::UserAborted => {
                ("aborted".to_string(), "user aborted (SIGINT)".to_string())
            }
        };
        Self {
            schema: "aura.report.v1".to_string(),
            instruction: instruction.to_string(),
            workspace: workspace.display().to_string(),
            status,
            used_turns: report.used_turns,
            summary,
            todo_final: report.todo_final.clone(),
        }
    }

    /// 序列化为 JSON 字符串。
    ///
    /// # Errors
    ///
    /// 序列化失败（结构错误，理论上不会发生）。
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

use std::path::Path;

/// 人类可读报告。
#[must_use]
pub fn format_text_report(instruction: &str, workspace: &Path, report: &RunReport) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "Aura run report");
    let _ = writeln!(s, "================");
    let _ = writeln!(s, "instruction: {instruction}");
    let _ = writeln!(s, "workspace:   {}", workspace.display());
    let _ = writeln!(s, "used_turns:  {}", report.used_turns);

    match &report.stop_reason {
        crate::agent::StopReasonPayload::Completed { summary } => {
            let _ = writeln!(s, "status:      OK");
            let _ = writeln!(s, "summary:     {summary}");
        }
        crate::agent::StopReasonPayload::ModelFailed { reason } => {
            let _ = writeln!(s, "status:      FAILED");
            let _ = writeln!(s, "reason:      model failed: {reason}");
        }
        crate::agent::StopReasonPayload::ModelAsked { question } => {
            let _ = writeln!(s, "status:      ASK");
            let _ = writeln!(s, "question:    {question}");
        }
        crate::agent::StopReasonPayload::BudgetExhausted { used } => {
            let _ = writeln!(s, "status:      FAILED");
            let _ = writeln!(s, "reason:      budget exhausted at turn {used}");
        }
        crate::agent::StopReasonPayload::ToolFailed { message } => {
            let _ = writeln!(s, "status:      FAILED");
            let _ = writeln!(s, "reason:      tool failed: {message}");
        }
        crate::agent::StopReasonPayload::UserAborted => {
            let _ = writeln!(s, "status:      ABORTED");
            let _ = writeln!(s, "reason:      user aborted (SIGINT)");
        }
    }
    s
}
