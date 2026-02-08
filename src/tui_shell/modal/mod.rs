use crossterm::event::KeyEvent;

use self::keymap::{ModalAction, map_modal_key};

mod draw;
mod keymap;
mod text_input_validate;

pub(super) use self::draw::draw_modal;

pub(super) fn handle_modal_key(app: &mut super::App, key: KeyEvent) {
    let action = {
        let Some(m) = app.modal_mut() else {
            return;
        };
        map_modal_key(m, key)
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
