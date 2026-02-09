use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListState, Paragraph, Wrap};

use super::{BundlesView, details, rows};
use crate::tui_shell::render_view_chrome;

pub(super) fn render(view: &BundlesView, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
    let inner = render_view_chrome(frame, "Bundles", &view.updated_at, area);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(inner);

    let mut state = ListState::default();
    if !view.items.is_empty() {
        state.select(Some(view.selected.min(view.items.len().saturating_sub(1))));
    }

    let list = List::new(rows::list_rows(view))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .title(rows::list_title(view)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));
    frame.render_stateful_widget(list, parts[0], &mut state);

    frame.render_widget(
        Paragraph::new(details::detail_lines(view)).wrap(Wrap { trim: false }),
        parts[1],
    );
}
