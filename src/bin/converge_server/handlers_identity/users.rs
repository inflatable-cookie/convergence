use super::*;

pub(crate) async fn list_users(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
) -> Result<Json<Vec<User>>, Response> {
    if !subject.admin {
        return Err(forbidden());
    }
    let users = state.users.read().await;
    let mut out: Vec<User> = users.values().cloned().collect();
    out.sort_by(|a, b| a.handle.cmp(&b.handle));
    Ok(Json(out))
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreateUserRequest {
    handle: String,

    #[serde(default)]
    display_name: Option<String>,

    #[serde(default)]
    admin: bool,
}

pub(crate) async fn create_user(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<User>, Response> {
    if !subject.admin {
        return Err(forbidden());
    }
    validate_user_handle(&payload.handle).map_err(bad_request)?;

    let created_at = now_ts();
    let user_id = generate_token_secret().map_err(internal_error)?;
    let user = User {
        id: user_id.clone(),
        handle: payload.handle.clone(),
        display_name: payload.display_name,
        admin: payload.admin,
        created_at,
    };

    {
        let mut users = state.users.write().await;
        if users.values().any(|u| u.handle == payload.handle) {
            return Err(conflict("user handle already exists"));
        }
        users.insert(user_id, user.clone());
    }

    {
        let users = state.users.read().await;
        let tokens = state.tokens.read().await;
        if let Err(err) = persist_identity_to_disk(&state.data_dir, &users, &tokens) {
            return Err(internal_error(err));
        }
    }

    Ok(Json(user))
}
