//! Wizard 状态机 — slice 1.5 stub。
//!
//! slice 3 接入第一个 provider 时实现：
//! - `App` 结构（当前状态、用户输入 buffer、状态枚举）
//! - `Message` 枚举（`Key` / `Char` / `Enter` / `Esc` / ...）
//! - `update(msg, &mut App)` 纯函数
//!
//! 当前仅占位 struct + `Default` 实现，编译通过即可。

/// Wizard 顶层状态。
///
/// slice 3 会扩展为 `enum State { PickProvider, EnterApiKey, Verifying, Saving, Done, Error, ... }`。
#[derive(Debug, Default, Clone)]
pub struct App {
    /// slice 1.5 标记：构造过 = 模块被装载。slice 3 删。
    _wired: bool,
}

impl App {
    /// 新建空 app（slice 1.5 默认值）。
    #[must_use]
    pub fn new() -> Self {
        Self { _wired: true }
    }
}
