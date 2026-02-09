use std::any::Any;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListState, Paragraph, Wrap};

use super::details::details_lines;
use super::rows::list_rows;
use super::*;

impl View for LanesView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Lanes
    }

    fn title(&self) -> &str {
        "Lanes"
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_down(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        let max = self.items.len().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, ctx: &RenderCtx) {
        let inner = render_view_chrome(frame, self.title(), self.updated_at(), area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        let mut state = ListState::default();
        if !self.items.is_empty() {
            state.select(Some(self.selected.min(self.items.len().saturating_sub(1))));
        }

        let list = List::new(list_rows(self, ctx))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .title("(Enter: fetch; /: commands)".to_string()),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        frame.render_widget(
            Paragraph::new(details_lines(self)).wrap(Wrap { trim: false }),
            parts[1],
        );
    }
}
