use super::*;

mod list;
mod mint;
mod revoke;
mod types;

pub(crate) use self::types::CreateTokenResponse;
use self::types::{CreateTokenRequest, TokenView};

pub(crate) async fn list_tokens(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
) -> Result<Json<Vec<TokenView>>, Response> {
    let out = list::list_tokens_for_subject(&state, &subject).await;
    Ok(Json(out))
}

pub(crate) async fn create_token(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Json(payload): Json<CreateTokenRequest>,
) -> Result<Json<CreateTokenResponse>, Response> {
    let out = mint::mint_token(&state, &subject.user_id, payload.label).await?;
    Ok(Json(out))
}

pub(crate) async fn create_token_for_user(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(user_id): Path<String>,
    Json(payload): Json<CreateTokenRequest>,
) -> Result<Json<CreateTokenResponse>, Response> {
    if !subject.admin && subject.user_id != user_id {
        return Err(forbidden());
    }
    {
        let users = state.users.read().await;
        if !users.contains_key(&user_id) {
            return Err(not_found());
        }
    }
    let out = mint::mint_token(&state, &user_id, payload.label).await?;
    Ok(Json(out))
}

pub(crate) async fn revoke_token(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(token_id): Path<String>,
) -> Result<Json<serde_json::Value>, Response> {
    revoke::revoke_token(&state, &subject, &token_id).await
}
