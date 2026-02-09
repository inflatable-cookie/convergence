use std::any::Any;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListState, Paragraph, Wrap};

use super::super::super::view::render_view_chrome_with_header;
use super::details::details_lines;
use super::rows::{list_rows, subtitle};
use super::{InboxView, RenderCtx, UiMode, View};

impl View for InboxView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Inbox
    }

    fn title(&self) -> &str {
        "Inbox"
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

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, _ctx: &RenderCtx) {
        let header = Line::from(vec![
            Span::styled(self.title().to_string(), Style::default().fg(Color::Yellow)),
            Span::raw("  "),
            Span::styled(
                format!(
                    "{} total  {} pending  {} resolved  {} missing",
                    self.total, self.pending, self.resolved, self.missing_local
                ),
                Style::default().fg(Color::Gray),
            ),
        ]);
        let inner = render_view_chrome_with_header(frame, header, area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        let mut state = ListState::default();
        if !self.items.is_empty() {
            state.select(Some(self.selected.min(self.items.len().saturating_sub(1))));
        }

        let list = List::new(list_rows(self))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .title(format!("{} (Enter: bundle; /: commands)", subtitle(self))),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        frame.render_widget(
            Paragraph::new(details_lines(self)).wrap(Wrap { trim: false }),
            parts[1],
        );
    }
}
