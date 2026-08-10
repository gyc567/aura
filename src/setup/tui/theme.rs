//! 颜色 + 样式 — slice 3 实现。
//!
//! 所有 UI 调这里，不写死 `Color::X`。换主题 = 改这一个文件。

use ratatui::style::{Color, Modifier, Style};

/// 主色（标题 / 强调）。
#[must_use]
pub fn primary() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// 次强调（可交互元素）。
#[must_use]
pub fn accent() -> Style {
    Style::default().fg(Color::Yellow)
}

/// 成功。
#[must_use]
pub fn success() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

/// 错误。
#[must_use]
pub fn error() -> Style {
    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
}
