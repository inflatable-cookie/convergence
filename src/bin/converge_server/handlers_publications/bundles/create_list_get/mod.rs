use super::super::super::*;

mod create;
mod create_helpers;
mod list_get;
pub(super) mod types;

pub(super) async fn create_bundle(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    repo_id: Path<String>,
    payload: Json<types::CreateBundleRequest>,
) -> Result<Json<Bundle>, Response> {
    create::create_bundle(state, subject, repo_id, payload).await
}

pub(super) async fn list_bundles(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    repo_id: Path<String>,
    q: Query<types::ListBundlesQuery>,
) -> Result<Json<Vec<Bundle>>, Response> {
    list_get::list_bundles(state, subject, repo_id, q).await
}

pub(super) async fn get_bundle(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    ids: Path<(String, String)>,
) -> Result<Json<Bundle>, Response> {
    list_get::get_bundle(state, subject, ids).await
}
