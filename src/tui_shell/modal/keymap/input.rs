use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn apply_input_edit_key(modal: &mut super::super::super::Modal, key: KeyEvent) {
    match key.code {
        KeyCode::Backspace => modal.input.backspace(),
        KeyCode::Delete => modal.input.delete(),
        KeyCode::Left => modal.input.move_left(),
        KeyCode::Right => modal.input.move_right(),
        KeyCode::Char(c) => {
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT)
            {
                modal.input.insert_char(c);
            }
        }
        _ => {}
    }
}
