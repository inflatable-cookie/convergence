use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::text_input_validate::{allow_empty_text_input, validate_text_input};

pub(super) enum ModalAction {
    None,
    Close,
    SubmitSnapMessage {
        snap_id: String,
        msg: String,
    },
    Confirm(super::super::app::PendingAction),
    SubmitTextInput {
        action: super::super::TextInputAction,
        value: String,
    },
}

pub(super) fn map_modal_key(modal: &mut super::super::Modal, key: KeyEvent) -> ModalAction {
    match &mut modal.kind {
        super::super::ModalKind::Viewer => handle_viewer_like_key(modal, key),

        super::super::ModalKind::SnapMessage { snap_id } => match key.code {
            KeyCode::Esc => ModalAction::Close,
            KeyCode::Enter => ModalAction::SubmitSnapMessage {
                snap_id: snap_id.clone(),
                msg: modal.input.buf.clone(),
            },
            _ => {
                apply_input_edit_key(modal, key);
                ModalAction::None
            }
        },

        super::super::ModalKind::TextInput { action, .. } => match key.code {
            KeyCode::Esc => ModalAction::Close,
            KeyCode::Enter => {
                let raw = modal.input.buf.trim().to_string();
                if raw.is_empty() && !allow_empty_text_input(action) {
                    append_modal_error(modal, "value required".to_string());
                    return ModalAction::None;
                }

                match validate_text_input(action, &raw) {
                    Ok(()) => ModalAction::SubmitTextInput {
                        action: action.clone(),
                        value: raw,
                    },
                    Err(msg) => {
                        append_modal_error(modal, msg);
                        ModalAction::None
                    }
                }
            }
            _ => {
                apply_input_edit_key(modal, key);
                ModalAction::None
            }
        },

        super::super::ModalKind::ConfirmAction { action } => match key.code {
            KeyCode::Esc => ModalAction::Close,
            KeyCode::Enter => ModalAction::Confirm(action.clone()),
            _ => handle_viewer_like_key(modal, key),
        },
    }
}

fn handle_viewer_like_key(modal: &mut super::super::Modal, key: KeyEvent) -> ModalAction {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => ModalAction::Close,
        KeyCode::Up => {
            modal.scroll = modal.scroll.saturating_sub(1);
            ModalAction::None
        }
        KeyCode::Down => {
            if modal.scroll < modal.lines.len().saturating_sub(1) {
                modal.scroll += 1;
            }
            ModalAction::None
        }
        KeyCode::PageUp => {
            modal.scroll = modal.scroll.saturating_sub(10);
            ModalAction::None
        }
        KeyCode::PageDown => {
            modal.scroll = (modal.scroll + 10).min(modal.lines.len().saturating_sub(1));
            ModalAction::None
        }
        _ => ModalAction::None,
    }
}

fn apply_input_edit_key(modal: &mut super::super::Modal, key: KeyEvent) {
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

fn append_modal_error(modal: &mut super::super::Modal, msg: String) {
    modal.lines.retain(|l| !l.starts_with("error:"));
    modal.lines.push(format!("error: {}", msg));
}
