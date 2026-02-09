use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub(super) fn render_modal_body(
    frame: &mut ratatui::Frame,
    modal: &super::super::super::Modal,
    inner: ratatui::layout::Rect,
) {
    match &modal.kind {
        super::super::super::ModalKind::Viewer
        | super::super::super::ModalKind::ConfirmAction { .. } => {
            render_lines(frame, modal, inner);
        }
        super::super::super::ModalKind::SnapMessage { .. } => {
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(inner);

            render_lines(frame, modal, parts[0]);
            frame.render_widget(
                Paragraph::new(modal.input.buf.as_str())
                    .block(Block::default().borders(Borders::ALL).title("Message")),
                parts[1],
            );
            let x = modal.input.cursor as u16;
            let y = parts[1].y + 1;
            frame.set_cursor_position((parts[1].x + 1 + x, y));
        }
        super::super::super::ModalKind::TextInput { prompt, .. } => {
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(inner);

            render_lines(frame, modal, parts[0]);

            let input_line = Line::from(vec![
                Span::styled(prompt.as_str(), Style::default().fg(Color::Yellow)),
                Span::raw(modal.input.buf.as_str()),
            ]);
            frame.render_widget(
                Paragraph::new(input_line)
                    .block(Block::default().borders(Borders::ALL).title("Edit")),
                parts[1],
            );

            let x = prompt.len() as u16 + modal.input.cursor as u16;
            let y = parts[1].y + 1;
            frame.set_cursor_position((parts[1].x + 1 + x, y));
        }
    }
}

fn render_lines(
    frame: &mut ratatui::Frame,
    modal: &super::super::super::Modal,
    area: ratatui::layout::Rect,
) {
    let lines: Vec<Line> = modal.lines.iter().map(|s| Line::from(s.as_str())).collect();
    let scroll = modal.scroll.min(modal.lines.len().saturating_sub(1)) as u16;
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}
