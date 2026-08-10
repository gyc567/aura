//! Ratatui-based TUI for the onboarding wizard.
//!
//! **slice 1.5 状态**：仅模块骨架与空 stub。后续 slice 会按
//! `docs/provider-onboarding.md` §6 接入完整 Elm 风格 app loop。
//!
//! 子模块：
//! - [`app`]   — Wizard 状态机 (`App` struct + `Message` enum + `update`)
//! - [`ui`]    — 纯渲染函数 (`render(frame, &app)`)，snapshot-testable
//! - [`event`] — Key/resize 事件 → `Message` 转换
//! - [`theme`] — 颜色 + 样式（单一来源）
//!
//! slice 3 之前 [`run`] 返回 `NotImplemented`，与 `super::run_wizard` 一致。

use std::io::Stdout;

use crate::error::AgentError;

pub mod app;
pub mod event;
pub mod theme;
pub mod ui;
/// 启动 TUI 向导（slice 1.5 stub：返回 `NotImplemented`）。
///
/// slice 3 起会替代 `super::run_wizard` 的 stub 行为；现版本仅做模块占位。
///
/// # Errors
///
/// slice 1.5: 永远返回 `AgentError::NotImplemented`。
pub fn run(_stdout: &mut Stdout) -> Result<(), AgentError> {
    Err(AgentError::NotImplemented(
        "setup::tui::run: ratatui skeleton wired in slice 1.5; first provider lands in slice 3"
            .into(),
    ))
}
