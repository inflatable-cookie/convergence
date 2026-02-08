use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub(in crate::tui_shell) fn draw_modal(frame: &mut ratatui::Frame, modal: &super::super::Modal) {
    let area = frame.area();
    let w = area.width.saturating_sub(6).clamp(20, 90);
    let h = area.height.saturating_sub(6).clamp(8, 22);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let box_area = ratatui::layout::Rect {
        x,
        y,
        width: w,
        height: h,
    };

    frame.render_widget(ratatui::widgets::Clear, box_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(modal_title(modal));
    frame.render_widget(block.clone(), box_area);
    let inner = block.inner(box_area);

    match &modal.kind {
        super::super::ModalKind::Viewer | super::super::ModalKind::ConfirmAction { .. } => {
            let lines: Vec<Line> = modal.lines.iter().map(|s| Line::from(s.as_str())).collect();
            let scroll = modal.scroll.min(modal.lines.len().saturating_sub(1)) as u16;
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                inner,
            );
        }

        super::super::ModalKind::SnapMessage { .. } => {
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(inner);

            let lines: Vec<Line> = modal.lines.iter().map(|s| Line::from(s.as_str())).collect();
            let scroll = modal.scroll.min(modal.lines.len().saturating_sub(1)) as u16;
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                parts[0],
            );

            frame.render_widget(
                Paragraph::new(modal.input.buf.as_str())
                    .block(Block::default().borders(Borders::ALL).title("Message")),
                parts[1],
            );
            let x = modal.input.cursor as u16;
            let y = parts[1].y + 1;
            frame.set_cursor_position((parts[1].x + 1 + x, y));
        }

        super::super::ModalKind::TextInput { prompt, .. } => {
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(inner);

            let lines: Vec<Line> = modal.lines.iter().map(|s| Line::from(s.as_str())).collect();
            let scroll = modal.scroll.min(modal.lines.len().saturating_sub(1)) as u16;
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                parts[0],
            );

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

fn modal_title(modal: &super::super::Modal) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            modal.title.as_str().to_string(),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  ".to_string()),
        Span::styled("Esc".to_string(), Style::default().fg(Color::Gray)),
    ];
    if matches!(
        &modal.kind,
        super::super::ModalKind::ConfirmAction { .. }
            | super::super::ModalKind::SnapMessage { .. }
            | super::super::ModalKind::TextInput { .. }
    ) {
        spans.push(Span::raw("  ".to_string()));
        spans.push(Span::styled(
            "Enter".to_string(),
            Style::default().fg(Color::Gray),
        ));
    }
    Line::from(spans)
}
