use super::*;

pub(super) fn draw(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(if app.suggestions.is_empty() { 0 } else { 9 }),
            Constraint::Length(3),
        ])
        .split(area);

    // Header
    let header_mid = if app.root_ctx == RootContext::Remote {
        app.workspace
            .as_ref()
            .and_then(|ws| ws.store.read_config().ok())
            .and_then(|c| c.remote)
            .map(|r| format!("repo={} scope={} gate={}", r.repo_id, r.scope, r.gate))
            .unwrap_or_else(|| "(no remote configured)".to_string())
    } else {
        app.workspace
            .as_ref()
            .map(|w| w.root.display().to_string())
            .or_else(|| app.workspace_err.clone())
            .unwrap_or_else(|| "(no workspace)".to_string())
    };

    let mut spans = vec![
        Span::styled(
            "Converge",
            Style::default().fg(Color::Black).bg(Color::White),
        ),
        Span::raw("  "),
        Span::styled(
            app.prompt(),
            Style::default().fg(root_ctx_color(app.root_ctx)),
        ),
        Span::raw("  "),
        Span::raw(header_mid),
    ];
    if let Some(id) = app.remote_identity.as_deref() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(id, Style::default().fg(Color::Green)));
    } else if let Some(note) = app.remote_identity_note.as_deref() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(note, Style::default().fg(Color::Red)));
    }

    let header = Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // Main view (modal)
    let ctx = RenderCtx {
        now: OffsetDateTime::now_utc(),
        ts_mode: app.ts_mode,
    };
    app.view().render(frame, chunks[1], &ctx);

    // Status / last result
    {
        let mut lines = Vec::new();
        if let Some(cmd) = &app.last_command {
            lines.push(Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::raw(cmd.as_str()),
            ]));
        }
        if let Some(r) = &app.last_result {
            let style = match r.kind {
                EntryKind::Output => Style::default().fg(Color::White),
                EntryKind::Error => Style::default().fg(Color::Red),
                EntryKind::Command => Style::default().fg(Color::Cyan),
            };
            for (i, l) in r.lines.iter().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{} ", fmt_ts_ui(&r.ts)),
                            Style::default().fg(Color::Gray),
                        ),
                        Span::styled(l.as_str(), style),
                    ]));
                } else {
                    lines.push(Line::from(Span::styled(l.as_str(), style)));
                }
            }
        }
        if lines.is_empty() {
            lines.push(Line::from(""));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::TOP).title("Last")),
            chunks[2],
        );
    }

    // Suggestions
    if !app.suggestions.is_empty() {
        let mut s_lines = Vec::new();
        let total = app.suggestions.len();
        let sel_idx = app
            .suggestion_selected
            .min(app.suggestions.len().saturating_sub(1));
        s_lines.push(Line::from(Span::styled(
            format!("Suggestions {}/{}", sel_idx + 1, total),
            Style::default().fg(Color::Gray),
        )));

        // Window suggestions to fit panel height and keep selection visible.
        let inner_h = chunks[3].height.saturating_sub(2) as usize; // top+bottom borders
        let max_items = inner_h.saturating_sub(1); // first line is title
        let max_items = max_items.max(1);
        let mut start = 0usize;
        if total > max_items {
            if sel_idx >= max_items {
                start = sel_idx + 1 - max_items;
            }
            start = start.min(total.saturating_sub(max_items));
        }
        let end = (start + max_items).min(total);

        for i in start..end {
            let s = &app.suggestions[i];
            let sel = i == sel_idx;
            let style = if sel {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            s_lines.push(Line::from(vec![
                Span::styled(format!("{: <10}", s.name), style.fg(Color::Yellow)),
                Span::styled(s.help, style.fg(Color::White)),
            ]));
        }
        let sugg =
            Paragraph::new(s_lines).block(Block::default().borders(Borders::TOP | Borders::BOTTOM));
        frame.render_widget(sugg, chunks[3]);
    }

    // Input
    let prompt = app.prompt();
    let buf = &app.input.buf;
    let prompt_color = root_ctx_color(app.root_ctx);

    let mut input_spans = Vec::new();
    input_spans.push(Span::styled(prompt, Style::default().fg(prompt_color)));
    input_spans.push(Span::raw(" "));
    input_spans.push(Span::raw(buf.as_str()));

    if let Some(hint) = input_hint_left(app) {
        // Keep hint separated from typed input.
        // If input is empty, avoid leading extra padding.
        let sep = if buf.is_empty() { "" } else { "  " };
        input_spans.push(Span::raw(sep));
        input_spans.push(Span::styled(
            hint,
            Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
        ));
    }

    let input_line = Line::from(input_spans);
    let input = Paragraph::new(input_line).block(Block::default().borders(Borders::TOP));
    frame.render_widget(input, chunks[4]);

    // Right-aligned hint (root context toggle)
    if let Some((hint_line, hint_len)) = input_hint_right(app) {
        let inner_w = chunks[4].width.saturating_sub(2) as usize;
        let left_len = prompt.len() + 1 + buf.len();
        let left_hint_len = input_hint_left(app)
            .map(|h| (if buf.is_empty() { 0 } else { 2 }) + h.len())
            .unwrap_or(0);
        let right_len = hint_len;
        // Only show if it doesn't collide with left content.
        if left_len + left_hint_len + 1 + right_len <= inner_w {
            let rect = ratatui::layout::Rect {
                x: chunks[4].x + 1,
                y: chunks[4].y + 1,
                width: chunks[4].width.saturating_sub(2),
                height: 1,
            };
            frame.render_widget(
                Paragraph::new(hint_line).alignment(ratatui::layout::Alignment::Right),
                rect,
            );
        }
    }

    // Cursor
    if let Some(m) = &app.modal {
        dim_frame(frame);
        modal::draw_modal(frame, m);
        return;
    }

    let x = prompt.len() as u16 + 1 + app.input.cursor as u16;
    let y = chunks[4].y + 1;
    frame.set_cursor_position((chunks[4].x + x, y));
}

fn dim_frame(frame: &mut ratatui::Frame) {
    let area = frame.area();
    let buf = frame.buffer_mut();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.modifier |= Modifier::DIM;
            }
        }
    }
}
