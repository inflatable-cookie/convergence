use super::*;

impl View for RootView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Root
    }

    fn title(&self) -> &str {
        match self.ctx {
            RootContext::Local => "Status",
            RootContext::Remote => "Dashboard",
        }
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn move_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    fn move_down(&mut self) {
        if self.scroll < self.lines.len().saturating_sub(1) {
            self.scroll += 1;
        }
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, _ctx: &RenderCtx) {
        let (inner, include_baseline_line) = match self.ctx {
            RootContext::Local => {
                let (header, keep_baseline_line) = local_header_and_baseline_line(self, area.width);
                (
                    render_view_chrome_with_header(frame, header, area),
                    keep_baseline_line,
                )
            }
            RootContext::Remote => {
                let header = Line::from(vec![
                    Span::styled(
                        self.title().to_string(),
                        Style::default().fg(root_ctx_color(RootContext::Remote)),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        fmt_ts_ui(self.updated_at()),
                        Style::default().fg(Color::Gray),
                    ),
                ]);
                let inner = render_view_chrome_with_header(frame, header, area);
                if let Some(lines) = self.remote_auth_block_lines.as_ref() {
                    frame.render_widget(
                        Paragraph::new(
                            lines
                                .iter()
                                .map(|s| Line::from(s.as_str()))
                                .collect::<Vec<_>>(),
                        )
                        .wrap(Wrap { trim: false }),
                        inner,
                    );
                    return;
                }
                if let Some(d) = &self.remote_dashboard {
                    render_remote_dashboard(frame, inner, d);
                    return;
                }

                let err = self.remote_err.as_deref().unwrap_or("dashboard: error");
                frame.render_widget(
                    Paragraph::new(vec![Line::from(err)])
                        .wrap(Wrap { trim: false })
                        .block(Block::default().borders(Borders::ALL).title("Dashboard")),
                    inner,
                );
                return;
            }
        };

        let mut lines = Vec::new();
        for s in &self.lines {
            if !include_baseline_line && s.trim_start().starts_with("baseline:") {
                continue;
            }
            lines.push(style_root_line(s));
        }
        if lines.is_empty() {
            lines.push(Line::from(""));
        }

        let scroll = self.scroll.min(lines.len().saturating_sub(1)) as u16;
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0)),
            inner,
        );
    }
}
