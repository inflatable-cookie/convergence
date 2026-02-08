//! DTOs and payload types for remote API requests/responses.

use std::collections::{HashMap, HashSet};

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MissingObjectsResponse {
    pub missing_blobs: Vec<String>,
    pub missing_manifests: Vec<String>,
    pub missing_recipes: Vec<String>,
    pub missing_snaps: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Pins {
    pub bundles: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct MissingObjectsRequest {
    pub(super) blobs: Vec<String>,
    pub(super) manifests: Vec<String>,
    pub(super) recipes: Vec<String>,
    pub(super) snaps: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct CreatePublicationRequest {
    pub(super) snap_id: String,
    pub(super) scope: String,
    pub(super) gate: String,

    #[serde(default, skip_serializing_if = "is_false")]
    pub(super) metadata_only: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) resolution: Option<PublicationResolution>,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct CreateRepoRequest {
    pub(super) id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Repo {
    pub id: String,
    pub owner: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BootstrapResponse {
    pub user: RemoteUser,
    pub token: CreateTokenResponse,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RepoMembers {
    pub owner: String,
    pub readers: Vec<String>,
    pub publishers: Vec<String>,

    #[serde(default)]
    pub owner_user_id: Option<String>,
    #[serde(default)]
    pub reader_user_ids: Vec<String>,
    #[serde(default)]
    pub publisher_user_ids: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LaneMembers {
    pub lane: String,
    pub members: Vec<String>,

    #[serde(default)]
    pub member_user_ids: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Publication {
    pub id: String,
    pub snap_id: String,
    pub scope: String,
    pub gate: String,
    pub publisher: String,
    pub created_at: String,

    #[serde(default)]
    pub resolution: Option<PublicationResolution>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PublicationResolution {
    pub bundle_id: String,
    pub root_manifest: String,
    pub resolved_root_manifest: String,
    pub created_at: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Bundle {
    pub id: String,
    pub scope: String,
    pub gate: String,
    pub root_manifest: String,
    pub input_publications: Vec<String>,
    pub created_by: String,
    pub created_at: String,
    pub promotable: bool,
    pub reasons: Vec<String>,

    #[serde(default)]
    pub approvals: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Promotion {
    pub id: String,
    pub bundle_id: String,
    pub scope: String,
    pub from_gate: String,
    pub to_gate: String,
    pub promoted_by: String,
    pub promoted_at: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Release {
    pub id: String,
    pub channel: String,
    pub bundle_id: String,
    pub scope: String,
    pub gate: String,
    pub released_by: String,

    #[serde(default)]
    pub released_by_user_id: Option<String>,

    pub released_at: String,

    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WhoAmI {
    pub user: String,
    pub user_id: String,
    pub admin: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TokenView {
    pub id: String,
    pub label: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CreateTokenResponse {
    pub id: String,
    pub token: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RemoteUser {
    pub id: String,
    pub handle: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub admin: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LaneHead {
    pub snap_id: String,
    pub updated_at: String,

    #[serde(default)]
    pub client_id: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Lane {
    pub id: String,
    pub members: HashSet<String>,

    #[serde(default)]
    pub heads: HashMap<String, LaneHead>,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct UpdateLaneHeadRequest {
    pub(super) snap_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) client_id: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GateGraph {
    pub version: u32,
    pub gates: Vec<GateDef>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GateDef {
    pub id: String,
    pub name: String,
    pub upstream: Vec<String>,

    #[serde(default = "default_true")]
    pub allow_releases: bool,

    #[serde(default)]
    pub allow_superpositions: bool,

    #[serde(default)]
    pub allow_metadata_only_publications: bool,

    #[serde(default)]
    pub required_approvals: u32,
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct GateGraphValidationError {
    pub(super) error: String,
    #[serde(default)]
    pub(super) issues: Vec<GateGraphIssueView>,
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct GateGraphIssueView {
    pub(super) code: String,
    pub(super) message: String,

    #[serde(default)]
    pub(super) gate: Option<String>,
    #[serde(default)]
    pub(super) upstream: Option<String>,
}
