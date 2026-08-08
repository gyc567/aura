//! `output` 模块单元测试。
//!
//! 覆盖：`src/output.rs` 中 `format_text_report` 和 `JsonReport` 的所有 `StopReasonPayload` 变体。

use aura::agent::{RunReport, StopReasonPayload};
use aura::output::{JsonReport, format_text_report};

fn make_report(stop_reason: StopReasonPayload, used_turns: u32) -> RunReport {
    RunReport {
        used_turns,
        stop_reason,
        todo_final: vec![],
    }
}

// ─── format_text_report ───────────────────────────────────────────────────────

#[test]
fn text_report_completed_shows_ok() {
    let report = make_report(
        StopReasonPayload::Completed {
            summary: "all done".into(),
        },
        2,
    );
    let output = format_text_report("fix bug", std::path::Path::new("/ws"), &report);
    assert!(output.contains("status:      OK"));
    assert!(output.contains("summary:     all done"));
    assert!(output.contains("used_turns:  2"));
}

#[test]
fn text_report_model_failed_shows_failed() {
    let report = make_report(
        StopReasonPayload::ModelFailed {
            reason: "invalid response".into(),
        },
        1,
    );
    let output = format_text_report("do thing", std::path::Path::new("/ws"), &report);
    assert!(output.contains("status:      FAILED"));
    assert!(output.contains("model failed: invalid response"));
}

#[test]
fn text_report_model_asked_shows_ask() {
    let report = make_report(
        StopReasonPayload::ModelAsked {
            question: "confirm?".into(),
        },
        1,
    );
    let output = format_text_report("do thing", std::path::Path::new("/ws"), &report);
    assert!(output.contains("status:      ASK"));
    assert!(output.contains("question:    confirm?"));
}

#[test]
fn text_report_budget_exhausted_shows_failed() {
    let report = make_report(StopReasonPayload::BudgetExhausted { used: 5 }, 5);
    let output = format_text_report("do thing", std::path::Path::new("/ws"), &report);
    assert!(output.contains("status:      FAILED"));
    assert!(output.contains("budget exhausted at turn 5"));
}

#[test]
fn text_report_tool_failed_shows_failed() {
    let report = make_report(
        StopReasonPayload::ToolFailed {
            message: "file not found".into(),
        },
        1,
    );
    let output = format_text_report("do thing", std::path::Path::new("/ws"), &report);
    assert!(output.contains("status:      FAILED"));
    assert!(output.contains("tool failed: file not found"));
}

#[test]
fn text_report_user_aborted_shows_aborted() {
    let report = make_report(StopReasonPayload::UserAborted, 3);
    let output = format_text_report("do thing", std::path::Path::new("/ws"), &report);
    assert!(output.contains("status:      ABORTED"));
    assert!(output.contains("user aborted (SIGINT)"));
}

// ─── JsonReport ────────────────────────────────────────────────────────────────

#[test]
fn json_report_completed_status_ok() {
    let report = make_report(
        StopReasonPayload::Completed {
            summary: "done".into(),
        },
        1,
    );
    let jr = JsonReport::from_report("fix bug", std::path::Path::new("/ws"), &report);
    assert_eq!(jr.status, "ok");
    assert_eq!(jr.summary, "done");
    assert_eq!(jr.used_turns, 1);
    assert_eq!(jr.schema, "aura.report.v1");
}

#[test]
fn json_report_model_failed_status_failed() {
    let report = make_report(
        StopReasonPayload::ModelFailed {
            reason: "bad".into(),
        },
        2,
    );
    let jr = JsonReport::from_report("do thing", std::path::Path::new("/ws"), &report);
    assert_eq!(jr.status, "failed");
    assert!(jr.summary.contains("model failed: bad"));
}

#[test]
fn json_report_model_asked_status_failed() {
    let report = make_report(
        StopReasonPayload::ModelAsked {
            question: "ok?".into(),
        },
        1,
    );
    let jr = JsonReport::from_report("do thing", std::path::Path::new("/ws"), &report);
    assert_eq!(jr.status, "failed");
    assert!(jr.summary.contains("model asked: ok?"));
}

#[test]
fn json_report_budget_exhausted_status_failed() {
    let report = make_report(StopReasonPayload::BudgetExhausted { used: 3 }, 3);
    let jr = JsonReport::from_report("do thing", std::path::Path::new("/ws"), &report);
    assert_eq!(jr.status, "failed");
    assert!(jr.summary.contains("budget exhausted"));
}

#[test]
fn json_report_tool_failed_status_failed() {
    let report = make_report(
        StopReasonPayload::ToolFailed {
            message: "oops".into(),
        },
        1,
    );
    let jr = JsonReport::from_report("do thing", std::path::Path::new("/ws"), &report);
    assert_eq!(jr.status, "failed");
    assert!(jr.summary.contains("tool failed: oops"));
}

#[test]
fn json_report_user_aborted_status_aborted() {
    let report = make_report(StopReasonPayload::UserAborted, 4);
    let jr = JsonReport::from_report("do thing", std::path::Path::new("/ws"), &report);
    assert_eq!(jr.status, "aborted");
    assert!(jr.summary.contains("user aborted"));
}

#[test]
fn json_report_to_json_produces_valid_json() {
    let report = make_report(
        StopReasonPayload::Completed {
            summary: "done".into(),
        },
        1,
    );
    let jr = JsonReport::from_report("fix bug", std::path::Path::new("/ws"), &report);
    let json = jr.to_json().expect("should serialize");
    assert!(json.contains("\"schema\": \"aura.report.v1\""));
    assert!(json.contains("\"status\": \"ok\""));
    // valid JSON parses back
    let _: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
}

#[test]
fn json_report_workspace_and_instruction_fields() {
    let report = make_report(
        StopReasonPayload::Completed {
            summary: "ok".into(),
        },
        1,
    );
    let jr = JsonReport::from_report("fix bug", std::path::Path::new("/my/workspace"), &report);
    assert_eq!(jr.instruction, "fix bug");
    assert_eq!(jr.workspace, "/my/workspace");
}
