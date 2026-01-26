use std::any::Any;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use super::super::{RenderCtx, UiMode, View, fmt_ts_ui, render_view_chrome};

#[derive(Debug)]
pub(in crate::tui_shell) struct InboxView {
    pub(in crate::tui_shell) updated_at: String,
    pub(in crate::tui_shell) scope: String,
    pub(in crate::tui_shell) gate: String,
    pub(in crate::tui_shell) filter: Option<String>,
    pub(in crate::tui_shell) limit: Option<usize>,
    pub(in crate::tui_shell) items: Vec<crate::remote::Publication>,
    pub(in crate::tui_shell) selected: usize,
}

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
        let inner = render_view_chrome(frame, self.title(), self.updated_at(), area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        let mut state = ListState::default();
        if !self.items.is_empty() {
            state.select(Some(self.selected.min(self.items.len().saturating_sub(1))));
        }

        let mut rows = Vec::new();
        for p in &self.items {
            let rid = p.id.chars().take(8).collect::<String>();
            let sid = p.snap_id.chars().take(8).collect::<String>();
            let res = if p.resolution.is_some() {
                " resolved"
            } else {
                ""
            };
            rows.push(ListItem::new(format!("{} {}{}", rid, sid, res)));
        }
        if rows.is_empty() {
            rows.push(ListItem::new("(empty)"));
        }

        let list = List::new(rows)
            .block(Block::default().borders(Borders::BOTTOM).title(format!(
                "scope={} gate={}{}{} (Enter: bundle; /: commands)",
                self.scope,
                self.gate,
                self.filter
                    .as_ref()
                    .map(|f| format!(" filter={}", f))
                    .unwrap_or_default(),
                self.limit
                    .map(|n| format!(" limit={}", n))
                    .unwrap_or_default()
            )))
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        let details = if self.items.is_empty() {
            vec![Line::from("(no selection)")]
        } else {
            let idx = self.selected.min(self.items.len().saturating_sub(1));
            let p = &self.items[idx];
            let mut out = Vec::new();
            out.push(Line::from(format!("id: {}", p.id)));
            out.push(Line::from(format!("snap: {}", p.snap_id)));
            out.push(Line::from(format!("publisher: {}", p.publisher)));
            out.push(Line::from(format!(
                "created_at: {}",
                fmt_ts_ui(&p.created_at)
            )));
            if let Some(r) = &p.resolution {
                out.push(Line::from(""));
                out.push(Line::from("resolution:"));
                out.push(Line::from(format!("  bundle_id: {}", r.bundle_id)));
            }
            out
        };
        frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
    }
}
