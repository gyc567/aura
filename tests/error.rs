//! `AgentError` 单元测试。
//!
//! 覆盖每个变体的 `Display` 与 `is_retryable`，以及 source 链路。

use aura::AgentError;

#[test]
fn display_includes_variant_and_message() {
    let err = AgentError::InvalidRequest("bad".into());
    let msg = err.to_string();
    assert!(msg.contains("invalid task request"));
    assert!(msg.contains("bad"));
}

#[test]
fn is_retryable_only_for_context() {
    assert!(AgentError::Context("io".into()).is_retryable());
    assert!(!AgentError::InvalidRequest("x".into()).is_retryable());
    assert!(!AgentError::InvalidTransition("x".into()).is_retryable());
    assert!(!AgentError::BudgetExhausted("x".into()).is_retryable());
    assert!(!AgentError::UnparseableDecision("x".into()).is_retryable());
    assert!(!AgentError::UnknownTool("x".into()).is_retryable());
    assert!(!AgentError::ToolFailed("x".into()).is_retryable());
    assert!(!AgentError::VerificationFailed("x".into()).is_retryable());
    assert!(!AgentError::PathPolicy("x".into()).is_retryable());
    assert!(!AgentError::CommandPolicy("x".into()).is_retryable());
    assert!(!AgentError::NeedsConfirmation("x".into()).is_retryable());
    assert!(!AgentError::InvalidArguments("x".into()).is_retryable());
}

#[test]
fn source_chain_present() {
    use std::error::Error;
    let err = AgentError::ToolFailed("boom".into());
    // thiserror 不附带 source；但保证 `Error::source` 存在（返回 None）。
    assert!(err.source().is_none());
}

#[test]
fn exit_code_for_policy_violations() {
    // PathPolicy / CommandPolicy / NeedsConfirmation → 3
    assert_eq!(AgentError::PathPolicy("bad".into()).exit_code(), 3);
    assert_eq!(AgentError::CommandPolicy("bad".into()).exit_code(), 3);
    assert_eq!(
        AgentError::NeedsConfirmation("confirm".into()).exit_code(),
        3
    );
}

#[test]
fn exit_code_for_other_errors() {
    // All other variants → 1
    assert_eq!(AgentError::InvalidRequest("bad".into()).exit_code(), 1);
    assert_eq!(AgentError::InvalidTransition("bad".into()).exit_code(), 1);
    assert_eq!(AgentError::BudgetExhausted("bad".into()).exit_code(), 1);
    assert_eq!(AgentError::UnparseableDecision("bad".into()).exit_code(), 1);
    assert_eq!(AgentError::UnknownTool("bad".into()).exit_code(), 1);
    assert_eq!(AgentError::ToolFailed("bad".into()).exit_code(), 1);
    assert_eq!(AgentError::VerificationFailed("bad".into()).exit_code(), 1);
    assert_eq!(AgentError::Context("bad".into()).exit_code(), 1);
    assert_eq!(AgentError::InvalidArguments("bad".into()).exit_code(), 1);
}
