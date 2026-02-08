use super::*;

mod bundles;
mod missing_objects;
mod pins;
mod publications;

pub(super) async fn find_missing_objects(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    repo_id: Path<String>,
    req: Json<missing_objects::MissingObjectsRequest>,
) -> Result<Json<missing_objects::MissingObjectsResponse>, Response> {
    missing_objects::find_missing_objects(state, subject, repo_id, req).await
}

pub(super) async fn create_publication(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    repo_id: Path<String>,
    payload: Json<publications::CreatePublicationRequest>,
) -> Result<Json<Publication>, Response> {
    publications::create_publication(state, subject, repo_id, payload).await
}

pub(super) async fn list_publications(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    repo_id: Path<String>,
) -> Result<Json<Vec<Publication>>, Response> {
    publications::list_publications(state, subject, repo_id).await
}

pub(super) async fn create_bundle(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    repo_id: Path<String>,
    payload: Json<bundles::CreateBundleRequest>,
) -> Result<Json<Bundle>, Response> {
    bundles::create_bundle(state, subject, repo_id, payload).await
}

pub(super) async fn list_bundles(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    repo_id: Path<String>,
    q: Query<bundles::ListBundlesQuery>,
) -> Result<Json<Vec<Bundle>>, Response> {
    bundles::list_bundles(state, subject, repo_id, q).await
}

pub(super) async fn get_bundle(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    ids: Path<(String, String)>,
) -> Result<Json<Bundle>, Response> {
    bundles::get_bundle(state, subject, ids).await
}

pub(super) async fn approve_bundle(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    ids: Path<(String, String)>,
) -> Result<Json<Bundle>, Response> {
    bundles::approve_bundle(state, subject, ids).await
}

pub(super) async fn list_pins(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    repo_id: Path<String>,
) -> Result<Json<serde_json::Value>, Response> {
    pins::list_pins(state, subject, repo_id).await
}

pub(super) async fn pin_bundle(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    ids: Path<(String, String)>,
) -> Result<Json<serde_json::Value>, Response> {
    pins::pin_bundle(state, subject, ids).await
}

pub(super) async fn unpin_bundle(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    ids: Path<(String, String)>,
) -> Result<Json<serde_json::Value>, Response> {
    pins::unpin_bundle(state, subject, ids).await
}
