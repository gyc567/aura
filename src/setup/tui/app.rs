//! Wizard 状态机 — slice 3 实现。
//!
//! Elm 架构：`update(msg, app) -> new app` 纯函数（不碰 IO），
//! `ui::render(frame, &app)` 纯渲染。IO（keychain save / config write）
//! 由 `setup::run_wizard` 在状态机驱动下执行。
//!
//! 状态：`PickProvider` → `EnterApiKey` → `Saving` → `Done` / `Error`。
//! `Verifying` 状态（slice 4 的 key probe）暂不实现。

use crate::setup::providers::Provider;

/// 状态机可转换的动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// 选择 provider（数字键或 Enter 确认）。
    SelectProvider(usize),
    /// API key 输入字符。
    Char(char),
    /// Backspace 删除最后一个字符。
    Backspace,
    /// 清空整个输入。
    Clear,
    /// 提交当前状态（Enter）。
    Submit,
    /// 取消 / 退出（Esc）。
    Cancel,
    /// IO 完成通知（commit 结果回来）。
    CommitDone(Result<(), String>),
}

/// Wizard 状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// 选 provider（高亮 + Enter）。
    PickProvider,
    /// 输入 API key（masked）。
    EnterApiKey,
    /// 正在写 keychain + config（IO 进行中）。
    Saving,
    /// 完成。
    Done,
    /// 可恢复错误。
    Error,
}

/// Wizard app 状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// 当前状态。
    pub state: State,
    /// provider 列表（不可变，来自 catalog）。
    pub providers: Vec<Provider>,
    /// 当前高亮 provider 下标。
    pub selected: usize,
    /// API key 输入 buffer（真实字符）。
    pub input: String,
    /// 错误消息（`State::Error` 时显示）。
    pub error: Option<String>,
}

impl App {
    /// 新建 app。
    #[must_use]
    pub fn new(providers: Vec<Provider>) -> Self {
        Self {
            state: State::PickProvider,
            providers,
            selected: 0,
            input: String::new(),
            error: None,
        }
    }

    /// 当前选中的 provider（slice 3: `PickProvider` 状态有效）。
    #[must_use]
    pub fn current_provider(&self) -> Option<&Provider> {
        self.providers.get(self.selected)
    }
}

/// 纯状态转移。
#[must_use]
pub fn update(msg: Message, mut app: App) -> App {
    match (msg, &app.state) {
        (Message::SelectProvider(i), State::PickProvider) if i < app.providers.len() => {
            app.selected = i;
            app.state = State::EnterApiKey;
        }
        (Message::Char(c), State::EnterApiKey) => {
            if c.is_control() {
                // 忽略控制字符
            } else {
                app.input.push(c);
            }
        }
        (Message::Backspace, State::EnterApiKey) => {
            app.input.pop();
        }
        (Message::Clear, State::EnterApiKey) => {
            app.input.clear();
        }
        (Message::Submit, State::EnterApiKey) => {
            if app.input.is_empty() {
                app.error = Some("API key cannot be empty".into());
                app.state = State::Error;
            } else {
                app.state = State::Saving;
            }
        }
        (Message::CommitDone(Ok(())), State::Saving) => {
            app.state = State::Done;
        }
        (Message::CommitDone(Err(e)), State::Saving) => {
            app.error = Some(e);
            app.state = State::Error;
        }
        // 其它状态下的消息：忽略（保幂等）
        _ => {}
    }
    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::providers::all;

    fn test_app() -> App {
        App::new(all().to_vec())
    }

    #[test]
    fn initial_state_is_pick_provider() {
        let app = test_app();
        assert_eq!(app.state, State::PickProvider);
        assert_eq!(app.selected, 0);
        assert_eq!(app.providers.len(), 4);
    }

    #[test]
    fn select_provider_transitions_to_enter_key() {
        let app = test_app();
        let app = update(Message::SelectProvider(1), app);
        assert_eq!(app.state, State::EnterApiKey);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn select_out_of_range_ignored() {
        let app = test_app();
        let app = update(Message::SelectProvider(99), app);
        assert_eq!(app.state, State::PickProvider);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn char_append_masked_input() {
        let app = test_app();
        let app = update(Message::SelectProvider(0), app);
        let mut app = app;
        app = update(Message::Char('s'), app);
        app = update(Message::Char('k'), app);
        assert_eq!(app.input, "sk");
    }

    #[test]
    fn backspace_removes_last_char() {
        let app = test_app();
        let app = update(Message::SelectProvider(0), app);
        let mut app = app;
        app = update(Message::Char('a'), app);
        app = update(Message::Char('b'), app);
        app = update(Message::Backspace, app);
        assert_eq!(app.input, "a");
    }

    #[test]
    fn clear_empties_input() {
        let app = test_app();
        let app = update(Message::SelectProvider(0), app);
        let mut app = app;
        app = update(Message::Char('x'), app);
        app = update(Message::Clear, app);
        assert_eq!(app.input, "");
    }

    #[test]
    fn submit_empty_input_errors() {
        let app = test_app();
        let app = update(Message::SelectProvider(0), app);
        let app = update(Message::Submit, app);
        assert_eq!(app.state, State::Error);
        assert!(app.error.is_some());
    }

    #[test]
    fn submit_nonempty_transitions_to_saving() {
        let app = test_app();
        let app = update(Message::SelectProvider(0), app);
        let app = update(Message::Char('k'), app);
        let app = update(Message::Submit, app);
        assert_eq!(app.state, State::Saving);
    }

    #[test]
    fn commit_ok_transitions_to_done() {
        let app = test_app();
        let app = update(Message::SelectProvider(0), app);
        let app = update(Message::Char('k'), app);
        let app = update(Message::Submit, app);
        let app = update(Message::CommitDone(Ok(())), app);
        assert_eq!(app.state, State::Done);
    }

    #[test]
    fn commit_err_transitions_to_error() {
        let app = test_app();
        let app = update(Message::SelectProvider(0), app);
        let app = update(Message::Char('k'), app);
        let app = update(Message::Submit, app);
        let app = update(Message::CommitDone(Err("keychain failed".into())), app);
        assert_eq!(app.state, State::Error);
        assert_eq!(app.error.as_deref(), Some("keychain failed"));
    }

    #[test]
    fn control_char_ignored() {
        let app = test_app();
        let app = update(Message::SelectProvider(0), app);
        let app = update(Message::Char('\u{1b}'), app); // ESC as char
        assert_eq!(app.input, "");
    }

    #[test]
    fn cancel_ignored_in_pick_provider() {
        // slice 3: Esc 在 PickProvider 由外部处理（退出 TUI），状态机内忽略
        let app = test_app();
        let app = update(Message::Cancel, app);
        assert_eq!(app.state, State::PickProvider);
    }
}
