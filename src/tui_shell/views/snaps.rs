use std::any::Any;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use super::super::{
    ChangeSummary, RenderCtx, UiMode, View, fmt_ts_list, fmt_ts_ui, render_view_chrome,
};

#[derive(Debug)]
pub(in crate::tui_shell) struct SnapsView {
    pub(in crate::tui_shell) updated_at: String,
    pub(in crate::tui_shell) filter: Option<String>,
    pub(in crate::tui_shell) all_items: Vec<crate::model::SnapRecord>,
    pub(in crate::tui_shell) items: Vec<crate::model::SnapRecord>,
    pub(in crate::tui_shell) selected: usize,

    pub(in crate::tui_shell) head_id: Option<String>,

    pub(in crate::tui_shell) pending_changes: Option<ChangeSummary>,
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
            let offset = if self.pending_changes.is_some() { 1 } else { 0 };
            state.select(Some(
                offset + self.selected.min(self.items.len().saturating_sub(1)),
            ));
        }

        let mut rows = Vec::new();

        let has_pending = self.pending_changes.is_some_and(|s| s.total() > 0);
        let head_style = Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD);

        if let Some(sum) = self.pending_changes
            && has_pending
        {
            let total = sum.total();
            let label = if total == 1 { "change" } else { "changes" };
            rows.push(ListItem::new(format!("> {} {}", total, label)).style(head_style));
        }

        for s in &self.items {
            let is_head = self.head_id.as_deref() == Some(s.id.as_str());
            let sid = s.id.chars().take(8).collect::<String>();
            let msg = s.message.clone().unwrap_or_default();
            let marker = if is_head { "*" } else { " " };
            let id_style = if is_head && !has_pending {
                head_style
            } else {
                Style::default()
            };
            let row = if msg.is_empty() {
                format!("{} {} {}", marker, sid, fmt_ts_list(&s.created_at, ctx))
            } else {
                format!(
                    "{} {} {} {}",
                    marker,
                    sid,
                    fmt_ts_list(&s.created_at, ctx),
                    msg
                )
            };

            rows.push(ListItem::new(row).style(id_style));
        }

        if self.items.is_empty() {
            rows.push(ListItem::new("(no snaps)"));
        }

        let list = List::new(rows)
            .block(Block::default().borders(Borders::BOTTOM).title(format!(
                "snaps{} (Enter: show; /: commands)",
                self.filter
                    .as_ref()
                    .map(|f| format!(" filter={}", f))
                    .unwrap_or_default()
            )))
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        let details = if self.items.is_empty() {
            vec![Line::from("(no selection)")]
        } else {
            let idx = self.selected.min(self.items.len().saturating_sub(1));
            let s = &self.items[idx];
            let mut out = Vec::new();
            if self.head_id.as_deref() == Some(s.id.as_str()) {
                out.push(Line::from("active: yes"));
            }
            out.push(Line::from(format!("id: {}", s.id)));
            out.push(Line::from(format!(
                "created_at: {}",
                fmt_ts_ui(&s.created_at)
            )));
            if let Some(msg) = &s.message
                && !msg.is_empty()
            {
                out.push(Line::from(format!("message: {}", msg)));
            }
            out.push(Line::from(format!(
                "root_manifest: {}",
                s.root_manifest.as_str()
            )));
            out.push(Line::from(format!(
                "stats: files={} dirs={} symlinks={} bytes={}",
                s.stats.files, s.stats.dirs, s.stats.symlinks, s.stats.bytes
            )));
            out
        };
        frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
    }
}
