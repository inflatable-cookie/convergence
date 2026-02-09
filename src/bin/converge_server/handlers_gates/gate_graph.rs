use super::*;

pub(crate) async fn list_gates(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<Vec<Gate>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }

    let gates = repo
        .gate_graph
        .gates
        .iter()
        .map(|g| Gate {
            id: g.id.clone(),
            name: g.name.clone(),
        })
        .collect();
    Ok(Json(gates))
}

pub(crate) async fn get_gate_graph(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<GateGraph>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }
    Ok(Json(repo.gate_graph.clone()))
}

pub(crate) async fn put_gate_graph(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(graph): Json<GateGraph>,
) -> Result<Json<GateGraph>, Response> {
    let issues = validate_gate_graph_issues(&graph);
    if !issues.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid gate graph", "issues": issues})),
        )
            .into_response());
    }

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !subject.admin {
        return Err(forbidden());
    }

    repo.gate_graph = graph.clone();
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(graph))
}
