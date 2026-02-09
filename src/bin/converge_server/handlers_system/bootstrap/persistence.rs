use super::*;

pub(super) async fn persist_identity(state: &Arc<AppState>) -> Result<(), Response> {
    let users = state.users.read().await;
    let tokens = state.tokens.read().await;
    if let Err(err) = persist_identity_to_disk(&state.data_dir, &users, &tokens) {
        return Err(internal_error(err));
    }
    Ok(())
}
