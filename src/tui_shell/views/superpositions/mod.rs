use std::any::Any;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListState, Paragraph, Wrap};

use crate::model::{ObjectId, ResolutionDecision};
use crate::resolve::ResolutionValidation;

use super::super::{RenderCtx, UiMode, View, render_view_chrome};

mod details;
mod rows;

use self::details::detail_lines;
use self::rows::list_rows;

#[derive(Debug)]
pub(in crate::tui_shell) struct SuperpositionsView {
    pub(in crate::tui_shell) updated_at: String,
    pub(in crate::tui_shell) bundle_id: String,
    pub(in crate::tui_shell) filter: Option<String>,
    pub(in crate::tui_shell) root_manifest: ObjectId,
    pub(in crate::tui_shell) variants:
        std::collections::BTreeMap<String, Vec<crate::model::SuperpositionVariant>>,
    pub(in crate::tui_shell) decisions: std::collections::BTreeMap<String, ResolutionDecision>,
    pub(in crate::tui_shell) validation: Option<ResolutionValidation>,
    pub(in crate::tui_shell) items: Vec<(String, usize)>,
    pub(in crate::tui_shell) selected: usize,
}

impl View for SuperpositionsView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Superpositions
    }

    fn title(&self) -> &str {
        "Superpositions"
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
        let inner = render_view_chrome(frame, self.title(), self.updated_at(), area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        let mut state = ListState::default();
        if !self.items.is_empty() {
            state.select(Some(self.selected.min(self.items.len().saturating_sub(1))));
        }

        let list = List::new(list_rows(self))
            .block(Block::default().borders(Borders::BOTTOM).title(format!(
                "bundle={}{}{} (pick; Alt+1..9, Alt+0; / for commands)",
                self.bundle_id.chars().take(8).collect::<String>(),
                self.filter
                    .as_ref()
                    .map(|f| format!(" filter={}", f))
                    .unwrap_or_default(),
                self.validation
                    .as_ref()
                    .map(|v| {
                        format!(
                            " missing={} invalid={}",
                            v.missing.len(),
                            v.invalid_keys.len() + v.out_of_range.len()
                        )
                    })
                    .unwrap_or_default()
            )))
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        frame.render_widget(
            Paragraph::new(detail_lines(self)).wrap(Wrap { trim: false }),
            parts[1],
        );
    }
}
