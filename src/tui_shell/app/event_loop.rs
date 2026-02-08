use super::*;

pub(super) fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut last_local_refresh = std::time::Instant::now();
    let local_refresh_interval = Duration::from_secs(3);
    loop {
        let should_auto_refresh_local = app.mode() == UiMode::Root
            && app.root_ctx == RootContext::Local
            && app.modal.is_none()
            && app.input.buf.is_empty()
            && last_local_refresh.elapsed() >= local_refresh_interval;
        if should_auto_refresh_local {
            app.refresh_root_view();
            last_local_refresh = std::time::Instant::now();
        }

        terminal
            .draw(|f| super::render::draw(f, app))
            .context("draw")?;
        if app.quit {
            return Ok(());
        }

        if event::poll(Duration::from_millis(50)).context("poll")? {
            match event::read().context("read event")? {
                Event::Key(k) if k.kind == KeyEventKind::Press => handle_key(app, k),
                _ => {}
            }
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
    if app.modal.is_some() {
        modal::handle_modal_key(app, key);
        return;
    }

    match key.code {
        KeyCode::Char('q') => {
            app.quit = true;
        }

        KeyCode::Esc => {
            if !app.input.buf.is_empty() {
                app.input.clear();
                app.recompute_suggestions();
            } else if app.mode() != UiMode::Root {
                app.pop_mode();
                app.push_output(vec![format!("back to {:?}", app.mode())]);
            } else {
                app.quit = true;
            }
        }

        KeyCode::Tab => {
            if app.input.buf.is_empty() {
                if app.root_ctx == RootContext::Local && app.mode() == UiMode::Root {
                    app.switch_to_remote_root();
                    app.push_output(vec!["switched to remote context".to_string()]);
                } else if app.root_ctx == RootContext::Remote {
                    app.switch_to_local_root();
                    app.push_output(vec!["switched to local context".to_string()]);
                }
            } else if !app.input.buf.is_empty() && !app.suggestions.is_empty() {
                app.apply_selected_suggestion();
            }
        }

        KeyCode::Enter => {
            if app.input.buf.is_empty() {
                app.run_default_action();
                return;
            }

            if !app.suggestions.is_empty() {
                let sel = app
                    .suggestion_selected
                    .min(app.suggestions.len().saturating_sub(1));
                let cmd = app.suggestions[sel].name;

                let raw = app.input.buf.trim_start_matches('/').trim_start();
                let first = raw.split_whitespace().next().unwrap_or("");
                if first != cmd {
                    app.apply_selected_suggestion();
                }
            }
            app.run_current_input();
        }

        KeyCode::Up => {
            if app.input.buf.is_empty() {
                app.view_mut().move_up();
                return;
            }
            if !app.suggestions.is_empty() {
                let n = app.suggestions.len();
                if n > 0 {
                    app.suggestion_selected = (app.suggestion_selected + n - 1) % n;
                }
                return;
            }
            app.input.history_up();
            app.recompute_suggestions();
        }
        KeyCode::Down => {
            if app.input.buf.is_empty() {
                app.view_mut().move_down();
                return;
            }
            if !app.suggestions.is_empty() {
                let n = app.suggestions.len();
                if n > 0 {
                    app.suggestion_selected = (app.suggestion_selected + 1) % n;
                }
                return;
            }
            app.input.history_down();
            app.recompute_suggestions();
        }

        KeyCode::Left => {
            if app.input.buf.is_empty() {
                app.rotate_hint(-1);
            } else {
                app.input.move_left();
            }
        }
        KeyCode::Right => {
            if app.input.buf.is_empty() {
                app.rotate_hint(1);
            } else {
                app.input.move_right();
            }
        }
        KeyCode::Backspace => {
            app.input.backspace();
            app.recompute_suggestions();
        }
        KeyCode::Delete => {
            app.input.delete();
            app.recompute_suggestions();
        }

        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.clear();
            app.recompute_suggestions();
        }

        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.history_up();
            app.recompute_suggestions();
        }

        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.history_down();
            app.recompute_suggestions();
        }

        KeyCode::Char(c)
            if key.modifiers.contains(KeyModifiers::ALT) && app.input.buf.is_empty() =>
        {
            if app.mode() == UiMode::Superpositions {
                if c.is_ascii_digit() {
                    let n = c.to_digit(10).unwrap_or(0) as usize;
                    // Alt+0 clears; Alt+1..9 selects variant.
                    if n == 0 {
                        super::superpositions_nav::superpositions_clear_decision(app);
                    } else {
                        super::superpositions_nav::superpositions_pick_variant(app, n - 1);
                    }
                }

                if c == 'f' {
                    super::superpositions_nav::superpositions_jump_next_invalid(app);
                }

                if c == 'n' {
                    super::superpositions_nav::superpositions_jump_next_missing(app);
                }
            }
        }

        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.insert_char(c);
            app.recompute_suggestions();
        }

        _ => {}
    }
}
