use super::types::{TokenView, to_token_view};
use super::*;

pub(super) async fn list_tokens_for_subject(
    state: &Arc<AppState>,
    subject: &Subject,
) -> Vec<TokenView> {
    let tokens = state.tokens.read().await;
    let mut out = Vec::new();
    for token in tokens.values() {
        if token.user_id != subject.user_id {
            continue;
        }
        out.push(to_token_view(token));
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}
