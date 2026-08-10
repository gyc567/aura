//! First-run onboarding wizard.
//!
//! 负责检测缺失的 provider 配置并引导用户完成 keychain 存储。
//! 详细设计见 [`docs/provider-onboarding.md`](../../docs/provider-onboarding.md)。
//!
//! **当前状态 (slice 1 — module skeleton)**：仅占位 API。
//! - [`needs_onboarding`] 永远返回 `false`，现有 `aura` 无参行为不变
//! - [`run_wizard`] 永远返回 "not implemented" 错误
//!
//! 后续 slice (1.5+) 会按设计文档逐步填充 ratatui TUI、provider catalog、keychain。

use std::process::ExitCode;

use crate::error::AgentError;

/// 当前是否需要进入 onboarding 向导。
///
/// **slice 1 实现**：永远返回 `false`。
/// 触发逻辑（CLI args / env / config / keychain 组合判断）在 slice 5 接入。
#[must_use]
pub fn needs_onboarding() -> bool {
    false
}

/// 后续 slice (1.5+) 会按设计文档逐步填充 ratatui TUI、provider catalog、keychain。
/// 启动 onboarding 向导（`aura setup` 子命令入口）。
///
/// slice 3 接入第一个 provider (`DeepSeek`) 的端到端流。
///
/// # Errors
///
/// slice 1: 永远返回 `AgentError::NotImplemented`。
pub fn run_wizard() -> Result<ExitCode, AgentError> {
    Err(AgentError::NotImplemented(
        "aura setup: onboarding wizard not yet implemented; tracked in docs/provider-onboarding.md slice 1.5+"
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// slice 1 承诺: `needs_onboarding` 永远为 `false`，保持现有行为不变。
    /// 这是 slice 5 之前 `aura <task>` 调用路径不破的契约。
    #[test]
    fn needs_onboarding_is_false_in_slice1() {
        assert!(!needs_onboarding());
    }

    /// slice 1 承诺: `run_wizard` 显式报 not-implemented 而不是 panic。
    /// slice 3 之前用户调用 `aura setup` 应该看到清晰错误，而不是无输出或崩溃。
    #[test]
    fn run_wizard_returns_not_implemented() {
        let err = run_wizard().unwrap_err();
        assert!(
            matches!(err, AgentError::NotImplemented(_)),
            "expected NotImplemented, got {err:?}",
        );
    }
}
