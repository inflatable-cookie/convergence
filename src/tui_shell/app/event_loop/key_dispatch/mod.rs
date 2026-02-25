use super::super::*;

mod input_edit;
mod movement;
mod root_mode;
mod superpositions_shortcuts;

pub(super) fn handle_key(app: &mut App, key: KeyEvent) {
    app.trace_key_action(key);

    if app.modal.is_some() {
        modal::handle_modal_key(app, key);
        return;
    }

    if input_edit::handle_input_edit_keys(app, key) {
        return;
    }

    match key.code {
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Esc => root_mode::handle_escape(app),
        KeyCode::Tab => root_mode::handle_tab(app),
        KeyCode::Enter => root_mode::handle_enter(app),
        KeyCode::Up => movement::handle_up(app),
        KeyCode::Down => movement::handle_down(app),
        KeyCode::Left => movement::handle_left(app),
        KeyCode::Right => movement::handle_right(app),
        KeyCode::Char(c)
            if key.modifiers.contains(KeyModifiers::ALT) && app.input.buf.is_empty() =>
        {
            superpositions_shortcuts::handle_alt_shortcut(app, c);
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.insert_char(c);
            app.recompute_suggestions();
        }
        _ => {}
    }
}
