use std::any::Any;

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

use time::OffsetDateTime;

#[derive(Clone, Copy, Debug)]
pub(super) struct RenderCtx {
    pub(super) now: OffsetDateTime,
    pub(super) ts_mode: super::TimestampMode,
}

pub(super) trait View: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn mode(&self) -> super::UiMode;
    fn title(&self) -> &str;
    fn updated_at(&self) -> &str;

    fn move_up(&mut self) {}
    fn move_down(&mut self) {}

    fn render(&self, frame: &mut ratatui::Frame, area: Rect, ctx: &RenderCtx);
}

pub(super) fn render_view_chrome(
    frame: &mut ratatui::Frame,
    title: &str,
    updated_at: &str,
    area: Rect,
) -> Rect {
    let header = Line::from(vec![
        Span::styled(title.to_string(), Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled(
            super::fmt_ts_ui(updated_at),
            Style::default().fg(Color::Gray),
        ),
    ]);

    render_view_chrome_with_header(frame, header, area)
}

pub(super) fn render_view_chrome_with_header<'a>(
    frame: &mut ratatui::Frame,
    header: Line<'a>,
    area: Rect,
) -> Rect {
    let outer = Block::default().borders(Borders::ALL).title(header);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    inner
}
