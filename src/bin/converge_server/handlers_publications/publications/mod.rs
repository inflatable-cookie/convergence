use super::super::*;

mod create;
mod list;
mod validate;

#[derive(Debug, serde::Deserialize)]
pub(in super::super) struct CreatePublicationRequest {
    snap_id: String,
    scope: String,
    gate: String,

    #[serde(default)]
    metadata_only: bool,

    #[serde(default)]
    resolution: Option<PublicationResolution>,
}

pub(super) async fn create_publication(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    repo_id: Path<String>,
    payload: Json<CreatePublicationRequest>,
) -> Result<Json<Publication>, Response> {
    create::create_publication(state, subject, repo_id, payload).await
}

pub(super) async fn list_publications(
    state: State<Arc<AppState>>,
    subject: Extension<Subject>,
    repo_id: Path<String>,
) -> Result<Json<Vec<Publication>>, Response> {
    list::list_publications(state, subject, repo_id).await
}
