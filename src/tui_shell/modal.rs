use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn draw_modal(frame: &mut ratatui::Frame, modal: &super::Modal) {
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
        super::ModalKind::Viewer | super::ModalKind::ConfirmAction { .. } => {
            let lines: Vec<Line> = modal.lines.iter().map(|s| Line::from(s.as_str())).collect();
            let scroll = modal.scroll.min(modal.lines.len().saturating_sub(1)) as u16;
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                inner,
            );
        }

        super::ModalKind::SnapMessage { .. } => {
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

        super::ModalKind::TextInput { prompt, .. } => {
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

pub(super) fn handle_modal_key(app: &mut super::App, key: KeyEvent) {
    enum ModalAction {
        None,
        Close,
        SubmitSnapMessage {
            snap_id: String,
            msg: String,
        },
        Confirm(super::app::PendingAction),
        SubmitTextInput {
            action: super::TextInputAction,
            value: String,
        },
    }

    let action = {
        let Some(m) = app.modal_mut() else {
            return;
        };

        match &mut m.kind {
            super::ModalKind::Viewer => match key.code {
                KeyCode::Esc | KeyCode::Enter => ModalAction::Close,
                KeyCode::Up => {
                    m.scroll = m.scroll.saturating_sub(1);
                    ModalAction::None
                }
                KeyCode::Down => {
                    if m.scroll < m.lines.len().saturating_sub(1) {
                        m.scroll += 1;
                    }
                    ModalAction::None
                }
                KeyCode::PageUp => {
                    m.scroll = m.scroll.saturating_sub(10);
                    ModalAction::None
                }
                KeyCode::PageDown => {
                    m.scroll = (m.scroll + 10).min(m.lines.len().saturating_sub(1));
                    ModalAction::None
                }
                _ => ModalAction::None,
            },
            super::ModalKind::SnapMessage { snap_id } => match key.code {
                KeyCode::Esc => ModalAction::Close,
                KeyCode::Enter => ModalAction::SubmitSnapMessage {
                    snap_id: snap_id.clone(),
                    msg: m.input.buf.clone(),
                },
                KeyCode::Backspace => {
                    m.input.backspace();
                    ModalAction::None
                }
                KeyCode::Delete => {
                    m.input.delete();
                    ModalAction::None
                }
                KeyCode::Left => {
                    m.input.move_left();
                    ModalAction::None
                }
                KeyCode::Right => {
                    m.input.move_right();
                    ModalAction::None
                }
                KeyCode::Char(c) => {
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT)
                    {
                        m.input.insert_char(c);
                    }
                    ModalAction::None
                }
                _ => ModalAction::None,
            },

            super::ModalKind::TextInput { action, .. } => match key.code {
                KeyCode::Esc => ModalAction::Close,
                KeyCode::Enter => {
                    let raw = m.input.buf.trim().to_string();
                    let allow_empty = matches!(
                        action,
                        super::TextInputAction::LoginScope
                            | super::TextInputAction::LoginGate
                            | super::TextInputAction::FetchId
                            | super::TextInputAction::FetchUser
                            | super::TextInputAction::FetchOptions
                            | super::TextInputAction::PublishSnap
                            | super::TextInputAction::PublishStart
                            | super::TextInputAction::PublishScope
                            | super::TextInputAction::PublishGate
                            | super::TextInputAction::PublishMeta
                            | super::TextInputAction::SyncStart
                            | super::TextInputAction::SyncLane
                            | super::TextInputAction::SyncClient
                            | super::TextInputAction::SyncSnap
                            | super::TextInputAction::ReleaseChannel
                            | super::TextInputAction::ReleaseNotes
                            | super::TextInputAction::PinAction
                            | super::TextInputAction::MemberRole
                            | super::TextInputAction::BrowseFilter
                            | super::TextInputAction::BrowseLimit
                    );
                    if raw.is_empty() && !allow_empty {
                        m.lines.retain(|l| !l.starts_with("error:"));
                        m.lines.push("error: value required".to_string());
                        return;
                    }

                    let validate = match action.clone() {
                        super::TextInputAction::ChunkingSet => {
                            let norm = raw.replace(',', " ");
                            let parts = norm.split_whitespace().collect::<Vec<_>>();
                            if parts.len() != 2 {
                                Err("format: <chunk_size_mib> <threshold_mib>".to_string())
                            } else {
                                let chunk = parts[0].parse::<u64>().ok();
                                let threshold = parts[1].parse::<u64>().ok();
                                match (chunk, threshold) {
                                    (Some(c), Some(t)) if c > 0 && t > 0 => {
                                        if t < c {
                                            Err("threshold must be >= chunk_size".to_string())
                                        } else {
                                            Ok(())
                                        }
                                    }
                                    _ => Err("invalid number".to_string()),
                                }
                            }
                        }
                        super::TextInputAction::RetentionKeepLast
                        | super::TextInputAction::RetentionKeepDays => {
                            let v = raw.to_lowercase();
                            if v == "unset" || v == "none" {
                                Ok(())
                            } else {
                                match raw.parse::<u64>() {
                                    Ok(n) if n > 0 => Ok(()),
                                    _ => Err("expected a positive number (or 'unset')".to_string()),
                                }
                            }
                        }

                        // Wizards / prompts.
                        super::TextInputAction::LoginUrl
                        | super::TextInputAction::LoginToken
                        | super::TextInputAction::LoginRepo
                        | super::TextInputAction::LoginScope
                        | super::TextInputAction::LoginGate => Ok(()),

                        super::TextInputAction::FetchKind
                        | super::TextInputAction::FetchId
                        | super::TextInputAction::FetchUser
                        | super::TextInputAction::FetchOptions
                        | super::TextInputAction::MoveFrom
                        | super::TextInputAction::MoveTo
                        | super::TextInputAction::PublishStart
                        | super::TextInputAction::PromoteToGate
                        | super::TextInputAction::PromoteBundleId
                        | super::TextInputAction::ReleaseBundleId
                        | super::TextInputAction::PinBundleId
                        | super::TextInputAction::PinAction
                        | super::TextInputAction::ApproveBundleId
                        | super::TextInputAction::SuperpositionsBundleId
                        | super::TextInputAction::MemberAction
                        | super::TextInputAction::MemberHandle
                        | super::TextInputAction::MemberRole
                        | super::TextInputAction::LaneMemberAction
                        | super::TextInputAction::LaneMemberLane
                        | super::TextInputAction::LaneMemberHandle
                        | super::TextInputAction::BrowseScope
                        | super::TextInputAction::BrowseGate
                        | super::TextInputAction::BrowseFilter
                        | super::TextInputAction::BrowseLimit
                        | super::TextInputAction::PublishSnap
                        | super::TextInputAction::PublishScope
                        | super::TextInputAction::PublishGate
                        | super::TextInputAction::PublishMeta => Ok(()),

                        super::TextInputAction::SyncStart
                        | super::TextInputAction::SyncLane
                        | super::TextInputAction::SyncClient
                        | super::TextInputAction::SyncSnap => Ok(()),

                        super::TextInputAction::ReleaseChannel
                        | super::TextInputAction::ReleaseNotes => Ok(()),
                    };

                    match validate {
                        Ok(()) => ModalAction::SubmitTextInput {
                            action: action.clone(),
                            value: raw,
                        },
                        Err(msg) => {
                            m.lines.retain(|l| !l.starts_with("error:"));
                            m.lines.push(format!("error: {}", msg));
                            ModalAction::None
                        }
                    }
                }
                KeyCode::Backspace => {
                    m.input.backspace();
                    ModalAction::None
                }
                KeyCode::Delete => {
                    m.input.delete();
                    ModalAction::None
                }
                KeyCode::Left => {
                    m.input.move_left();
                    ModalAction::None
                }
                KeyCode::Right => {
                    m.input.move_right();
                    ModalAction::None
                }
                KeyCode::Char(c) => {
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT)
                    {
                        m.input.insert_char(c);
                    }
                    ModalAction::None
                }
                _ => ModalAction::None,
            },

            super::ModalKind::ConfirmAction { action } => match key.code {
                KeyCode::Esc => ModalAction::Close,
                KeyCode::Enter => ModalAction::Confirm(action.clone()),
                KeyCode::Up => {
                    m.scroll = m.scroll.saturating_sub(1);
                    ModalAction::None
                }
                KeyCode::Down => {
                    if m.scroll < m.lines.len().saturating_sub(1) {
                        m.scroll += 1;
                    }
                    ModalAction::None
                }
                KeyCode::PageUp => {
                    m.scroll = m.scroll.saturating_sub(10);
                    ModalAction::None
                }
                KeyCode::PageDown => {
                    m.scroll = (m.scroll + 10).min(m.lines.len().saturating_sub(1));
                    ModalAction::None
                }
                _ => ModalAction::None,
            },
        }
    };

    match action {
        ModalAction::None => {}
        ModalAction::Close => {
            app.close_modal();
            app.cancel_wizards();
        }
        ModalAction::SubmitSnapMessage { snap_id, msg } => {
            app.close_modal();
            let Some(ws) = app.require_workspace() else {
                return;
            };
            let msg = msg.trim().to_string();
            let msg = if msg.is_empty() { None } else { Some(msg) };
            if let Err(err) = ws.store.update_snap_message(&snap_id, msg.as_deref()) {
                app.push_error(format!("set message: {:#}", err));
                return;
            }

            // Refresh snaps view list (if visible) and root status.
            if let Some(v) = app.current_view_mut::<super::views::SnapsView>() {
                let selected_id = v
                    .selected_snap_index()
                    .and_then(|i| v.items.get(i))
                    .map(|s| s.id.clone());

                match ws.list_snaps() {
                    Ok(snaps) => {
                        v.all_items = snaps.clone();
                        v.items = snaps;
                        v.head_id = ws.store.get_head().ok().flatten();
                        if let Some(sel) = selected_id
                            && let Some(i) = v.items.iter().position(|s| s.id == sel)
                        {
                            let has_header = v.pending_changes.is_some_and(|s| s.total() > 0)
                                || (v.pending_changes.is_none() && v.head_id.is_some());
                            v.selected_row = i + if has_header { 1 } else { 0 };
                        }
                        v.updated_at = super::app::now_ts();
                    }
                    Err(err) => {
                        app.push_error(format!("list snaps: {:#}", err));
                    }
                }
            }

            app.refresh_root_view();
            app.push_output(vec!["updated snap message".to_string()]);
        }

        ModalAction::Confirm(action) => {
            app.close_modal();
            // Confirmed actions should not re-prompt.
            app.execute_action_confirmed(action);
        }

        ModalAction::SubmitTextInput { action, value } => {
            app.close_modal();
            app.submit_text_input(action, value);
        }
    }
}

fn modal_title(modal: &super::Modal) -> Line<'static> {
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
        super::ModalKind::ConfirmAction { .. }
            | super::ModalKind::SnapMessage { .. }
            | super::ModalKind::TextInput { .. }
    ) {
        spans.push(Span::raw("  ".to_string()));
        spans.push(Span::styled(
            "Enter".to_string(),
            Style::default().fg(Color::Gray),
        ));
    }
    Line::from(spans)
}
