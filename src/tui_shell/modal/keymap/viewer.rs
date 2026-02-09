use crossterm::event::{KeyCode, KeyEvent};

use super::ModalAction;

pub(super) fn handle_viewer_like_key(
    modal: &mut super::super::super::Modal,
    key: KeyEvent,
) -> ModalAction {
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
