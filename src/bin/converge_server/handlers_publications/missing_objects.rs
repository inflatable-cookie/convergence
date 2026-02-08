use super::super::*;

#[derive(Debug, serde::Deserialize)]
pub(in super::super) struct MissingObjectsRequest {
    blobs: Vec<String>,
    manifests: Vec<String>,
    recipes: Vec<String>,
    snaps: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub(in super::super) struct MissingObjectsResponse {
    missing_blobs: Vec<String>,
    missing_manifests: Vec<String>,
    missing_recipes: Vec<String>,
    missing_snaps: Vec<String>,
}

pub(super) async fn find_missing_objects(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(req): Json<MissingObjectsRequest>,
) -> Result<Json<MissingObjectsResponse>, Response> {
    {
        let repos = state.repos.read().await;
        let repo = repos.get(&repo_id).ok_or_else(not_found)?;
        if !can_publish(repo, &subject) {
            return Err(forbidden());
        }
    }

    for id in req
        .blobs
        .iter()
        .chain(req.manifests.iter())
        .chain(req.recipes.iter())
        .chain(req.snaps.iter())
    {
        validate_object_id(id).map_err(bad_request)?;
    }

    let root = repo_data_dir(&state, &repo_id).join("objects");

    let mut missing_blobs = Vec::new();
    for id in req.blobs {
        let p = root.join("blobs").join(&id);
        if !p.exists() {
            missing_blobs.push(id);
        }
    }

    let mut missing_manifests = Vec::new();
    for id in req.manifests {
        let p = root.join("manifests").join(format!("{}.json", id));
        if !p.exists() {
            missing_manifests.push(id);
        }
    }

    let mut missing_recipes = Vec::new();
    for id in req.recipes {
        let p = root.join("recipes").join(format!("{}.json", id));
        if !p.exists() {
            missing_recipes.push(id);
        }
    }

    let mut missing_snaps = Vec::new();
    for id in req.snaps {
        let p = root.join("snaps").join(format!("{}.json", id));
        if !p.exists() {
            missing_snaps.push(id);
        }
    }

    Ok(Json(MissingObjectsResponse {
        missing_blobs,
        missing_manifests,
        missing_recipes,
        missing_snaps,
    }))
}
