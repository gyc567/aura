//! 终端事件 → `app::Message` 转换 — slice 3 实现。
//!
//! 纯函数 `key_to_message`：crossterm `KeyEvent` → 可选 `Message`。
//! 分状态处理（`PickProvider` 的数字键 vs `EnterApiKey` 的字符输入）。

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{Message, State};

/// 把 crossterm key event 转为状态机消息。
///
/// 返回 `None` = 该 key 在本状态无动作（忽略）。
#[must_use]
pub fn key_to_message(key: KeyEvent, state: &State) -> Option<Message> {
    match state {
        State::PickProvider => match key.code {
            KeyCode::Char('1') => Some(Message::SelectProvider(0)),
            KeyCode::Char('2') => Some(Message::SelectProvider(1)),
            KeyCode::Char('3') => Some(Message::SelectProvider(2)),
            KeyCode::Char('4') => Some(Message::SelectProvider(3)),
            KeyCode::Char('5') => Some(Message::SelectProvider(4)),
            KeyCode::Esc => Some(Message::Cancel),
            _ => None,
        },
        State::EnterApiKey => match key.code {
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'u' => {
                Some(Message::Clear)
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                Some(Message::Char(c))
            }
            KeyCode::Backspace => Some(Message::Backspace),
            KeyCode::Enter => Some(Message::Submit),
            KeyCode::Esc => Some(Message::Cancel),
            _ => None,
        },
        State::Saving | State::Done | State::Error => match key.code {
            KeyCode::Esc => Some(Message::Cancel),
            _ => None,
        },
    }
}

/// `Message::Cancel` 是否应终止 TUI（外部处理）。
///
/// `PickProvider` 的 Cancel = 退出；`EnterApiKey` 的 Cancel = 返回 provider 选择
/// （slice 3: 直接退出，简化 —— slice 4+ 再处理"返回上一步"）。
#[must_use]
pub fn cancel_quits(msg: &Message) -> bool {
    matches!(msg, Message::Cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn pick_provider_number_keys() {
        let state = State::PickProvider;
        assert_eq!(
            key_to_message(key(KeyCode::Char('1')), &state),
            Some(Message::SelectProvider(0))
        );
        assert_eq!(
            key_to_message(key(KeyCode::Char('4')), &state),
            Some(Message::SelectProvider(3))
        );
    }

    #[test]
    fn pick_provider_out_of_range_ignored() {
        let state = State::PickProvider;
        assert_eq!(key_to_message(key(KeyCode::Char('9')), &state), None);
        assert_eq!(key_to_message(key(KeyCode::Char('a')), &state), None);
    }

    #[test]
    fn enter_key_chars_forwarded() {
        let state = State::EnterApiKey;
        assert_eq!(
            key_to_message(key(KeyCode::Char('s')), &state),
            Some(Message::Char('s'))
        );
    }

    #[test]
    fn enter_key_backspace_enter_esc() {
        let state = State::EnterApiKey;
        assert_eq!(
            key_to_message(key(KeyCode::Backspace), &state),
            Some(Message::Backspace)
        );
        assert_eq!(
            key_to_message(key(KeyCode::Enter), &state),
            Some(Message::Submit)
        );
        assert_eq!(
            key_to_message(key(KeyCode::Esc), &state),
            Some(Message::Cancel)
        );
    }

    #[test]
    fn ctrl_u_clears() {
        let state = State::EnterApiKey;
        let ev = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(key_to_message(ev, &state), Some(Message::Clear));
    }

    #[test]
    fn ctrl_c_not_forwarded_as_char() {
        let state = State::EnterApiKey;
        let ev = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        // Ctrl-C 不进 input（避免把控制序列当字符）；顶层 SIGINT handler 处理
        assert_ne!(key_to_message(ev, &state), Some(Message::Char('c')));
    }

    #[test]
    fn cancel_quits_semantics() {
        assert!(cancel_quits(&Message::Cancel));
        assert!(!cancel_quits(&Message::Submit));
    }
}
