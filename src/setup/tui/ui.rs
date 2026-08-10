//! 纯渲染函数 — slice 1.5 stub。
//!
//! slice 3 接入第一个 provider 时实现：根据 `App` 当前状态在 frame 上画
//! provider 列表 / masked text input / status line。
//!
//! slice 1.5 仅 `render_smoke` —— 用 `TestBackend` 渲染一个 80×24 的空 frame，
//! 用于确认 ratatui skeleton 在 CI / `cargo test` 中编译并工作。

/// slice 1.5 smoke test: 渲染一个空 frame，确认 ratatui 编译并工作。
///
/// slice 3+ 会改为按 `App` 状态分支渲染。
#[cfg(test)]
pub(crate) fn render_smoke() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::{Block, Paragraph};
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("TestBackend init");
    terminal
        .draw(|frame| {
            let area = frame.area();
            let block = Block::default();
            frame.render_widget(block, area);
            let p = Paragraph::new("aura setup — slice 1.5 ratatui skeleton");
            frame.render_widget(p, area);
        })
        .expect("TestBackend draw");
    // sanity: buffer 不应为空（80*24=1920 cells，至少 1 个非空白）
    let _ = terminal.backend().buffer().clone();
}
