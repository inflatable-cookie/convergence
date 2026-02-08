use super::*;

pub(super) fn input_hint_left(app: &App) -> Option<String> {
    if !app.input.buf.is_empty() {
        return None;
    }
    if app.modal.is_some() {
        return None;
    }

    let cmds = app.primary_hint_commands();
    if cmds.is_empty() {
        return None;
    }

    Some(cmds.join(" | "))
}

pub(super) fn input_hint_right(app: &App) -> Option<(Line<'static>, usize)> {
    if !app.input.buf.is_empty() {
        return None;
    }
    if app.modal.is_some() {
        return None;
    }
    if app.mode() != UiMode::Root {
        return None;
    }

    match app.root_ctx {
        RootContext::Local => Some((
            Line::from(vec![
                Span::styled(
                    "Tab:".to_string(),
                    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                ),
                Span::raw(" "),
                Span::styled("remote".to_string(), Style::default().fg(Color::Blue)),
            ]),
            "Tab: remote".len(),
        )),
        RootContext::Remote => Some((
            Line::from(vec![
                Span::styled(
                    "Tab:".to_string(),
                    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                ),
                Span::raw(" "),
                Span::styled("local".to_string(), Style::default().fg(Color::Yellow)),
            ]),
            "Tab: local".len(),
        )),
    }
}
