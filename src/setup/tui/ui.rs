//! 纯渲染函数 — slice 3 实现。
//!
//! 每个 `App` 状态对应一个渲染分支；函数是纯的（只读 `App`，写 frame），
//! 可用 `TestBackend` snapshot-test。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use super::app::{App, State};
use super::theme;

/// 渲染当前状态（TUI 主入口）。
pub fn render(frame: &mut Frame<'_>, app: &App) {
    match app.state {
        State::PickProvider => render_pick_provider(frame, app),
        State::EnterApiKey => render_enter_key(frame, app),
        State::Saving => render_saving(frame, app),
        State::Done => render_done(frame, app),
        State::Error => render_error(frame, app),
    }
}

fn render_pick_provider(frame: &mut Frame<'_>, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .split(frame.area());

    let title = Paragraph::new(Line::from(vec![
        Span::styled("Aura 0.2", theme::primary()),
        Span::raw(" — first-time setup: pick a provider"),
    ]));
    frame.render_widget(title, chunks[0]);

    let items: Vec<ListItem<'_>> = app
        .providers
        .iter()
        .enumerate()
        .map(|(i, p)| {
            ListItem::new(Line::from(vec![
                Span::raw(format!("{}  ", i + 1)),
                Span::raw(p.display_name.clone()),
                Span::styled(
                    format!("  ({})", p.endpoint),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Providers "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, chunks[1], &mut app_state(app));

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("↑/↓ or 1-4 to select, "),
            Span::styled("Enter", theme::accent()),
            Span::raw(" to confirm, "),
            Span::styled("Esc", theme::accent()),
            Span::raw(" to quit"),
        ])),
        chunks[2],
    );
}

fn render_enter_key(frame: &mut Frame<'_>, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());

    let provider = app.current_provider().map(|p| p.display_name.clone());
    let title = Paragraph::new(Line::from(vec![
        Span::styled("Enter API key", theme::primary()),
        Span::raw(" for "),
        Span::styled(provider.unwrap_or_default(), theme::accent()),
    ]));
    frame.render_widget(title, chunks[0]);

    // masked input: 每个字符渲染为 *
    let masked: String = "*".repeat(app.input.chars().count());
    let input_block = Block::default().borders(Borders::ALL).title(" API key ");
    let input = Paragraph::new(Line::from(Span::raw(masked)))
        .block(input_block)
        .style(Style::default().fg(Color::White));
    frame.render_widget(input, chunks[1]);
    let cursor_col = chunks[1]
        .x
        .saturating_add(2)
        .saturating_add(u16::try_from(app.input.chars().count()).unwrap_or(u16::MAX));
    frame.set_cursor_position((cursor_col, chunks[1].y + 1));

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("paste your key, "),
            Span::styled("Enter", theme::accent()),
            Span::raw(" to save, "),
            Span::styled("Ctrl-U", theme::accent()),
            Span::raw(" to clear, "),
            Span::styled("Esc", theme::accent()),
            Span::raw(" to abort"),
        ])),
        chunks[3],
    );
}

fn render_saving(frame: &mut Frame<'_>, app: &App) {
    let provider = app.current_provider().map(|p| p.display_name.clone());
    let text = Text::from(vec![
        Line::from(Span::styled("Saving…", theme::primary())),
        Line::from(Span::raw(format!(
            "writing key for {} to keychain + config",
            provider.unwrap_or_default()
        ))),
    ]);
    frame.render_widget(Paragraph::new(text), frame.area());
}

fn render_done(frame: &mut Frame<'_>, app: &App) {
    let provider = app.current_provider().map(|p| p.display_name.clone());
    let text = Text::from(vec![
        Line::from(Span::styled("✓ Done", theme::success())),
        Line::from(Span::raw(format!(
            "API key for {} saved. Next: run `aura \"<task>\"`",
            provider.unwrap_or_default()
        ))),
        Line::from(Span::raw("[ press any key to exit ]")),
    ]);
    frame.render_widget(Paragraph::new(text), frame.area());
}

fn render_error(frame: &mut Frame<'_>, app: &App) {
    let text = Text::from(vec![
        Line::from(Span::styled("Error", theme::error())),
        Line::from(Span::raw(app.error.clone().unwrap_or_default())),
        Line::from(Span::raw("[ Esc to quit, Enter to retry ]")),
    ]);
    frame.render_widget(Paragraph::new(text), frame.area());
}

/// 给 `List` 用的 stateful selection。
fn app_state(app: &App) -> ratatui::widgets::ListState {
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.selected));
    state
}

/// slice 1.5 smoke test 保留（向后兼容）。
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
    let _ = terminal.backend().buffer().clone();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::providers::all;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// 渲染每个状态不 panic + 输出非空。
    fn render_state(app: &App) -> Vec<ratatui::buffer::Cell> {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("init");
        terminal.draw(|f| render(f, app)).expect("draw");
        terminal.backend().buffer().content().to_vec()
    }

    #[test]
    fn pick_provider_renders_nonempty() {
        let app = App::new(all().to_vec());
        let cells = render_state(&app);
        let non_blank = cells.iter().filter(|c| c.symbol() != " ").count();
        assert!(non_blank > 10, "expected visible content, got {non_blank}");
    }

    #[test]
    fn enter_key_masks_input() {
        use ratatui::layout::Rect;

        let app = App::new(all().to_vec());
        let app = super::super::app::update(super::super::app::Message::SelectProvider(0), app);
        let app = super::super::app::update(super::super::app::Message::Char('s'), app);
        let app = super::super::app::update(super::super::app::Message::Char('k'), app);

        // 渲染并抓 input block 区域（chunks[1]：第 1 行 3 高后的 3 高区域）
        let mut input_area: Option<Rect> = None;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("init");
        terminal
            .draw(|f| {
                let chunks = ratatui::layout::Layout::vertical([
                    ratatui::layout::Constraint::Length(3),
                    ratatui::layout::Constraint::Length(3),
                    ratatui::layout::Constraint::Min(0),
                    ratatui::layout::Constraint::Length(1),
                ])
                .split(f.area());
                // 只渲染 enter_key 分支（避免提示文案干扰）
                render_enter_key(f, &app);
                // 记录 input 区域
                input_area = Some(chunks[1]);
            })
            .expect("draw");

        let area = input_area.expect("input area");
        let mut stars = 0;
        let mut raw_s = 0;
        let mut raw_k = 0;
        // 跳过 border 行（y==area.y 是上边框，含 block 标题 "API key" 的 'k'）
        for y in area.y + 1..area.y + area.height - 1 {
            for x in area.x + 1..area.x + area.width - 1 {
                let cell = terminal
                    .backend()
                    .buffer()
                    .cell((x, y))
                    .expect("cell in bounds");
                match cell.symbol() {
                    "*" => stars += 1,
                    "s" => raw_s += 1,
                    "k" => raw_k += 1,
                    _ => {}
                }
            }
        }
        assert!(stars >= 2, "expected masked stars, got {stars}");
        assert_eq!(raw_s, 0, "plain 's' leaked in input area!");
        assert_eq!(raw_k, 0, "plain 'k' leaked in input area!");
    }

    #[test]
    fn saving_renders_nonempty() {
        let app = App::new(all().to_vec());
        let app = super::super::app::update(super::super::app::Message::SelectProvider(0), app);
        let app = super::super::app::update(super::super::app::Message::Char('k'), app);
        let app = super::super::app::update(super::super::app::Message::Submit, app);
        let cells = render_state(&app);
        let non_blank = cells.iter().filter(|c| c.symbol() != " ").count();
        assert!(non_blank > 5);
    }

    #[test]
    fn done_renders_success() {
        let app = App::new(all().to_vec());
        let app = super::super::app::update(super::super::app::Message::SelectProvider(0), app);
        let app = super::super::app::update(super::super::app::Message::Char('k'), app);
        let app = super::super::app::update(super::super::app::Message::Submit, app);
        let app = super::super::app::update(super::super::app::Message::CommitDone(Ok(())), app);
        let cells = render_state(&app);
        let non_blank = cells.iter().filter(|c| c.symbol() != " ").count();
        assert!(non_blank > 5);
    }

    #[test]
    fn error_renders_message() {
        let app = App::new(all().to_vec());
        let app = super::super::app::update(super::super::app::Message::SelectProvider(0), app);
        let app = super::super::app::update(super::super::app::Message::Char('k'), app);
        let app = super::super::app::update(super::super::app::Message::Submit, app);
        let app = super::super::app::update(
            super::super::app::Message::CommitDone(Err("boom".into())),
            app,
        );
        let cells = render_state(&app);
        // error message 'boom' 应出现
        let has_boom = cells.iter().any(|c| c.symbol() == "b" || c.symbol() == "o");
        assert!(has_boom);
    }
}
