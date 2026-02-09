use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListState, Paragraph, Wrap};

use super::details::detail_lines;
use super::list_rows::list_rows;
use super::view::SettingsView;
use crate::tui_shell::render_view_chrome;
use crate::tui_shell::view::RenderCtx;

pub(super) fn render(
    view: &SettingsView,
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    ctx: &RenderCtx,
) {
    let inner = render_view_chrome(frame, "Settings", &view.updated_at, area);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(inner);

    let mut state = ListState::default();
    if !view.items.is_empty() {
        state.select(Some(view.selected.min(view.items.len().saturating_sub(1))));
    }

    let list = List::new(list_rows(view, ctx))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .title("(Enter: do it; /: commands)"),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));
    frame.render_stateful_widget(list, parts[0], &mut state);

    frame.render_widget(
        Paragraph::new(detail_lines(view, ctx)).wrap(Wrap { trim: false }),
        parts[1],
    );
}
