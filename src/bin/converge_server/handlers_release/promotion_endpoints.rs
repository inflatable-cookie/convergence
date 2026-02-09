use super::*;

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreatePromotionRequest {
    bundle_id: String,
    to_gate: String,
}

pub(crate) async fn create_promotion(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(payload): Json<CreatePromotionRequest>,
) -> Result<Json<Promotion>, Response> {
    validate_object_id(&payload.bundle_id).map_err(bad_request)?;
    validate_gate_id(&payload.to_gate).map_err(bad_request)?;

    let promoted_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    let bundle = if let Some(b) = repo.bundles.iter().find(|b| b.id == payload.bundle_id) {
        b.clone()
    } else {
        load_bundle_from_disk(state.as_ref(), &repo_id, &payload.bundle_id)?
    };

    // Re-check promotability at promotion time.
    let gate_def = repo
        .gate_graph
        .gates
        .iter()
        .find(|g| g.id == bundle.gate)
        .ok_or_else(|| internal_error(anyhow::anyhow!("bundle gate not found")))?;
    let has_superpositions =
        manifest_has_superpositions(state.as_ref(), &repo_id, &bundle.root_manifest)?;
    let (promotable, _reasons) =
        compute_promotability(gate_def, has_superpositions, bundle.approvals.len());
    if !promotable {
        return Err(conflict("bundle not promotable"));
    }

    // Validate gate relationship: to_gate must list bundle.gate as upstream.
    let to_gate_def = repo
        .gate_graph
        .gates
        .iter()
        .find(|g| g.id == payload.to_gate)
        .ok_or_else(|| bad_request(anyhow::anyhow!("unknown to_gate")))?;
    if !to_gate_def.upstream.iter().any(|u| u == &bundle.gate) {
        return Err(bad_request(anyhow::anyhow!(
            "to_gate is not downstream of bundle gate"
        )));
    }

    let id = {
        let mut hasher = blake3::Hasher::new();
        hasher.update(repo_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(bundle.id.as_bytes());
        hasher.update(b"\n");
        hasher.update(bundle.scope.as_bytes());
        hasher.update(b"\n");
        hasher.update(bundle.gate.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.to_gate.as_bytes());
        hasher.update(b"\n");
        hasher.update(subject.user.as_bytes());
        hasher.update(b"\n");
        hasher.update(promoted_at.as_bytes());
        hasher.finalize().to_hex().to_string()
    };

    let promotion = Promotion {
        id: id.clone(),
        bundle_id: bundle.id.clone(),
        scope: bundle.scope.clone(),
        from_gate: bundle.gate.clone(),
        to_gate: payload.to_gate,
        promoted_by: subject.user.clone(),
        promoted_by_user_id: Some(subject.user_id.clone()),
        promoted_at,
    };

    // Update state pointer.
    repo.promotion_state
        .entry(promotion.scope.clone())
        .or_default()
        .insert(promotion.to_gate.clone(), promotion.bundle_id.clone());

    // Persist promotion record.
    let bytes =
        serde_json::to_vec_pretty(&promotion).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let path = repo_data_dir(&state, &repo_id)
        .join("promotions")
        .join(format!("{}.json", id));
    write_if_absent(&path, &bytes).map_err(internal_error)?;

    repo.promotions.push(promotion.clone());
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(promotion))
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ListPromotionsQuery {
    scope: Option<String>,
    to_gate: Option<String>,
}

pub(crate) async fn list_promotions(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Query(q): Query<ListPromotionsQuery>,
) -> Result<Json<Vec<Promotion>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }

    let mut out = Vec::new();
    for p in &repo.promotions {
        if let Some(scope) = &q.scope
            && &p.scope != scope
        {
            continue;
        }
        if let Some(to_gate) = &q.to_gate
            && &p.to_gate != to_gate
        {
            continue;
        }
        out.push(p.clone());
    }
    Ok(Json(out))
}
