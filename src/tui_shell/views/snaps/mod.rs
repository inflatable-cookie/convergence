use std::any::Any;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListState, Paragraph, Wrap};

use super::super::status::ChangeSummary;
use super::super::{RenderCtx, UiMode, View, render_view_chrome};

mod details;
mod rows;

use self::details::details_lines;
use self::rows::list_rows;

#[derive(Debug)]
pub(in crate::tui_shell) struct SnapsView {
    pub(in crate::tui_shell) updated_at: String,
    pub(in crate::tui_shell) filter: Option<String>,
    pub(in crate::tui_shell) all_items: Vec<crate::model::SnapRecord>,
    pub(in crate::tui_shell) items: Vec<crate::model::SnapRecord>,
    pub(in crate::tui_shell) selected_row: usize,

    pub(in crate::tui_shell) head_id: Option<String>,

    pub(in crate::tui_shell) pending_changes: Option<ChangeSummary>,
}

impl SnapsView {
    fn has_pending_row(&self) -> bool {
        self.pending_changes.is_some_and(|s| s.total() > 0)
    }

    fn has_clean_row(&self) -> bool {
        self.pending_changes.is_none() && self.head_id.is_some() && !self.all_items.is_empty()
    }

    fn has_header_row(&self) -> bool {
        self.has_pending_row() || self.has_clean_row()
    }

    fn rows_len(&self) -> usize {
        let mut n = self.items.len();
        if self.has_header_row() {
            n += 1;
        }
        if self.items.is_empty() {
            n += 1;
        }
        n
    }

    pub(in crate::tui_shell) fn selected_is_pending(&self) -> bool {
        self.has_pending_row() && self.selected_row.min(self.rows_len().saturating_sub(1)) == 0
    }

    pub(in crate::tui_shell) fn selected_is_clean(&self) -> bool {
        self.has_clean_row() && self.selected_row.min(self.rows_len().saturating_sub(1)) == 0
    }

    pub(in crate::tui_shell) fn selected_snap_index(&self) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let row = self.selected_row.min(self.rows_len().saturating_sub(1));
        let idx = if self.has_header_row() {
            if row == 0 {
                return None;
            }
            row - 1
        } else {
            row
        };
        if idx < self.items.len() {
            Some(idx)
        } else {
            None
        }
    }
}

impl View for SnapsView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Snaps
    }

    fn title(&self) -> &str {
        "History"
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn move_up(&mut self) {
        self.selected_row = self.selected_row.saturating_sub(1);
    }

    fn move_down(&mut self) {
        let n = self.rows_len();
        if n == 0 {
            self.selected_row = 0;
            return;
        }
        let max = n.saturating_sub(1);
        self.selected_row = (self.selected_row + 1).min(max);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, ctx: &RenderCtx) {
        let inner = render_view_chrome(frame, self.title(), self.updated_at(), area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        let mut state = ListState::default();
        let n_rows = self.rows_len();
        if n_rows > 0 {
            state.select(Some(self.selected_row.min(n_rows - 1)));
        }

        let list = List::new(list_rows(self, ctx))
            .block(Block::default().borders(Borders::BOTTOM).title(format!(
                "snaps{} (/: commands)",
                self.filter
                    .as_ref()
                    .map(|f| format!(" filter={}", f))
                    .unwrap_or_default()
            )))
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        frame.render_widget(
            Paragraph::new(details_lines(self, ctx)).wrap(Wrap { trim: false }),
            parts[1],
        );
    }
}

pub(super) fn head_style() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}
