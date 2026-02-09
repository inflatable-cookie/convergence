use super::*;

#[derive(Debug, serde::Deserialize)]
pub(crate) struct PromotionStateQuery {
    scope: String,
}

pub(crate) async fn get_promotion_state(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Query(q): Query<PromotionStateQuery>,
) -> Result<Json<HashMap<String, String>>, Response> {
    validate_scope_id(&q.scope).map_err(bad_request)?;
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }
    Ok(Json(
        repo.promotion_state
            .get(&q.scope)
            .cloned()
            .unwrap_or_default(),
    ))
}
