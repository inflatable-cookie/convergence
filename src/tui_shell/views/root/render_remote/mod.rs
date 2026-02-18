use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui_shell::status::DashboardData;

mod sections;

pub(super) fn render_remote_dashboard(frame: &mut ratatui::Frame, area: Rect, d: &DashboardData) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(area);

    frame.render_widget(
        Paragraph::new(sections::action_lines(d))
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Next")),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .split(cols[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .split(cols[1]);

    frame.render_widget(
        Paragraph::new(sections::inbox_lines(d))
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Inbox")),
        left[0],
    );

    frame.render_widget(
        Paragraph::new(sections::bundle_lines(d))
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Bundles")),
        left[1],
    );

    frame.render_widget(
        Paragraph::new(sections::gate_lines(d))
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Gates")),
        right[0],
    );

    frame.render_widget(
        Paragraph::new(sections::release_lines(d))
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Releases")),
        right[1],
    );
}
