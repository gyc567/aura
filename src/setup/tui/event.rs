//! 事件 → Message 转换 — slice 1.5 stub。
//!
//! slice 3+ 实现：监听 `crossterm::event::EventStream`，把 `KeyEvent` 转成
//! `app::Message::Key(code)` / `Message::Char(c)` / `Message::Enter` / `Message::Esc`。
//!
//! 当前模块为空 — 公开 API 尚未冻结（避免 slice 1.5 写出 slice 3 才会用的 API 表面）。

// 模块有意为空。slice 3 接入第一个 provider 时填充。
