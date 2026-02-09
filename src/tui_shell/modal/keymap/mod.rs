use crossterm::event::{KeyCode, KeyEvent};

use self::errors::append_modal_error;
use self::input::apply_input_edit_key;
use self::viewer::handle_viewer_like_key;
use super::text_input_validate::{allow_empty_text_input, validate_text_input};

mod errors;
mod input;
mod viewer;

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
