//! First-run onboarding wizard.
//!
//! 负责检测缺失的 provider 配置并引导用户完成 keychain 存储。
//! 详细设计见 [`docs/provider-onboarding.md`](../../docs/provider-onboarding.md)。
//!
//! **当前状态 (slice 3)**：TUI provider picker + API key 输入 + keychain/config 落盘。
//! - [`needs_onboarding`] 仍返回 `false`（slice 5 接无参触发）
//! - [`run_wizard`] 已实现：`PickProvider → EnterApiKey → Saving → Done`
//! - `Verifying`（key probe，slice 4）未接入；`aura` 无参触发（slice 5）未接入

use std::io::IsTerminal;
use std::process::ExitCode;

use crate::error::AgentError;

pub mod config_write;
pub mod keychain;
pub mod providers;
pub mod tui;

/// 当前是否需要进入 onboarding 向导。
///
/// **slice 1 实现**：永远返回 `false`。
/// 触发逻辑（CLI args / env / config / keychain 组合判断）在 slice 5 接入。
#[must_use]
pub fn needs_onboarding() -> bool {
    false
}

/// 启动 onboarding 向导（`aura setup` 子命令入口）。
///
/// slice 3：完整 TUI 事件循环。退出码：
/// - `0` — 完成（keychain + config 已写）
/// - `130` — 用户 Esc / Ctrl-C 取消
///
/// # Errors
///
/// - `AgentError::ToolFailed` — keychain 写入失败 / config 写入失败
pub fn run_wizard() -> Result<ExitCode, AgentError> {
    run_wizard_with(providers::all().to_vec(), &commit)
}

/// 用给定 provider 列表 + commit 回调跑向导（测试可注入）。
///
/// `commit` 签名 `(provider, key) -> Result<(), String>`：slice 3 用
/// [`commit`]（keychain + config），测试可换 mock。
fn run_wizard_with(
    providers: Vec<providers::Provider>,
    commit_fn: &dyn Fn(&providers::Provider, &str) -> Result<(), String>,
) -> Result<ExitCode, AgentError> {
    use std::io::stdout;

    // 非 TTY 环境：直接报错（设计 §6.5）
    if !std::io::stdin().is_terminal() || !stdout().is_terminal() {
        return Err(AgentError::InvalidRequest(
            "no TTY detected: run `aura setup` interactively, or set AURA_API_KEY and pass --api-key".into(),
        ));
    }

    // ratatui::init: raw mode + alternate screen + panic hook 全包（推荐用法）
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, providers, commit_fn);
    ratatui::restore(); // 无论成功失败都恢复终端

    result
}

/// 事件循环：read → update → render，直到 Done / Cancel。
fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    providers: Vec<providers::Provider>,
    commit_fn: &dyn Fn(&providers::Provider, &str) -> Result<(), String>,
) -> Result<ExitCode, AgentError> {
    use crossterm::event::{Event, read};

    let mut app = tui::app::App::new(providers);

    loop {
        terminal
            .draw(|f| tui::ui::render(f, &app))
            .map_err(|e| AgentError::ToolFailed(format!("render: {e}")))?;

        match read() {
            Ok(Event::Key(key)) => {
                let Some(msg) = tui::event::key_to_message(key, &app.state) else {
                    continue;
                };

                // Cancel = 退出（slice 3 简化：任意状态 Esc 都退出）
                if matches!(msg, tui::app::Message::Cancel) {
                    return Ok(ExitCode::from(130));
                }

                let next = tui::app::update(msg, app);
                app = next;

                // Saving 状态：同步跑 commit（slice 3 无异步 IO）
                if app.state == tui::app::State::Saving {
                    let provider = app.current_provider().expect("provider in Saving");
                    let key = app.input.clone();
                    let result = commit_fn(provider, &key);
                    app = tui::app::update(tui::app::Message::CommitDone(result), app);
                }
            }
            Ok(_) => { /* Resize 重渲染由 draw 自动处理；slice 3 不支持鼠标 */ }
            Err(e) => {
                return Err(AgentError::ToolFailed(format!("event read: {e}")));
            }
        }

        // Done 状态：按任意键退出
        if app.state == tui::app::State::Done {
            loop {
                if matches!(read(), Ok(Event::Key(_))) {
                    break;
                }
            }
            return Ok(ExitCode::SUCCESS);
        }
    }
}

/// slice 3 默认 commit：写 keychain（service = `provider.keychain_service`）
/// 再写 config（endpoint / model / provider）。原子性：keychain 成功才写 config。
fn commit(provider: &providers::Provider, key: &str) -> Result<(), String> {
    keychain::save(&provider.keychain_service, key).map_err(|e| e.to_string())?;
    config_write::write(&provider.endpoint, &provider.default_model, &provider.id)
        .map_err(|e| e.to_string())?;
    Ok(())
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

    /// slice 3 承诺: `run_wizard` 在非 TTY 环境（CI / 测试）报清晰错误，不 panic、不卡死。
    /// 这同时验证了设计 §6.5 的 non-TTY fallback。
    #[test]
    fn run_wizard_non_tty_errors_cleanly() {
        let err = run_wizard().unwrap_err();
        assert!(
            matches!(err, AgentError::InvalidRequest(_)),
            "expected InvalidRequest (non-TTY), got {err:?}",
        );
    }

    /// slice 3 承诺: `run_wizard_with` 的事件循环在注入 mock commit 时能完成状态流转。
    /// 不真正开 TTY —— 只测 `update` 驱动的状态机 + commit 回调（纯逻辑路径）。
    #[test]
    fn wizard_state_flow_with_mock_commit() {
        let providers = providers::all().to_vec();
        let mut app = tui::app::App::new(providers);
        // PickProvider → EnterApiKey
        app = tui::app::update(tui::app::Message::SelectProvider(0), app);
        assert_eq!(app.state, tui::app::State::EnterApiKey);
        // 输入 key
        for c in "sk-test".chars() {
            app = tui::app::update(tui::app::Message::Char(c), app);
        }
        // Submit → Saving
        app = tui::app::update(tui::app::Message::Submit, app);
        assert_eq!(app.state, tui::app::State::Saving);
        // mock commit 成功 → Done
        app = tui::app::update(tui::app::Message::CommitDone(Ok(())), app);
        assert_eq!(app.state, tui::app::State::Done);
    }

    /// slice 1.5 承诺: ratatui skeleton 在 `cargo test` 中能编译并 render 一个空 frame。
    /// 这条 test 跑成功 = 跨平台 CI 五矩阵（linux/macOS/windows x64+arm64）能编译 ratatui。
    /// 失败 = slice 1.5 没准备好，需先排查 MSRV / 后端兼容性问题再进 slice 3。
    #[test]
    fn tui_renders_empty_frame() {
        tui::ui::render_smoke();
    }

    /// slice 3 承诺: `App` 可用 provider catalog 构造（状态机纯函数，无 IO）。
    #[test]
    fn tui_app_new_accepts_provider_list() {
        let _app = tui::app::App::new(providers::all().to_vec());
    }
}
