use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

pub(super) fn modal_title(modal: &super::super::super::Modal) -> Line<'static> {
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
        super::super::super::ModalKind::ConfirmAction { .. }
            | super::super::super::ModalKind::SnapMessage { .. }
            | super::super::super::ModalKind::TextInput { .. }
    ) {
        spans.push(Span::raw("  ".to_string()));
        spans.push(Span::styled(
            "Enter".to_string(),
            Style::default().fg(Color::Gray),
        ));
    }
    Line::from(spans)
}
