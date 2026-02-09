use super::super::*;

mod auth;
mod create;
mod persistence;

#[derive(Debug, serde::Deserialize)]
pub(crate) struct BootstrapRequest {
    handle: String,

    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct BootstrapResponse {
    user: User,
    token: CreateTokenResponse,
}

pub(super) async fn bootstrap(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<BootstrapRequest>,
) -> Result<Json<BootstrapResponse>, Response> {
    auth::verify_bootstrap_bearer(&state, &headers)?;
    validate_user_handle(&payload.handle).map_err(bad_request)?;

    let created_at = now_ts();
    let user = create::create_admin_user(&payload, &created_at)?;
    create::assert_bootstrap_window_open(&state).await?;
    create::insert_user(&state, user.clone()).await?;

    let (token_id, token_secret) =
        create::create_bootstrap_token(&state, &user, &created_at).await?;
    persistence::persist_identity(&state).await?;

    Ok(Json(BootstrapResponse {
        user,
        token: CreateTokenResponse {
            id: token_id,
            token: token_secret,
            created_at,
        },
    }))
}
