use super::*;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Repo {
    pub(crate) id: String,
    pub(crate) owner: String,

    #[serde(default)]
    pub(crate) owner_user_id: Option<String>,

    pub(crate) readers: HashSet<String>,

    #[serde(default)]
    pub(crate) reader_user_ids: HashSet<String>,

    pub(crate) publishers: HashSet<String>,

    #[serde(default)]
    pub(crate) publisher_user_ids: HashSet<String>,

    pub(crate) lanes: HashMap<String, Lane>,

    pub(crate) gate_graph: GateGraph,
    pub(crate) scopes: HashSet<String>,

    pub(crate) snaps: HashSet<String>,
    pub(crate) publications: Vec<Publication>,

    pub(crate) bundles: Vec<Bundle>,

    #[serde(default)]
    pub(crate) pinned_bundles: HashSet<String>,

    pub(crate) promotions: Vec<Promotion>,
    pub(crate) promotion_state: HashMap<String, HashMap<String, String>>,

    #[serde(default)]
    pub(crate) releases: Vec<Release>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Gate {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct GateGraph {
    pub(crate) version: u32,
    pub(crate) gates: Vec<GateDef>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct GateDef {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) upstream: Vec<String>,

    #[serde(default = "default_true")]
    pub(crate) allow_releases: bool,

    #[serde(default)]
    pub(crate) allow_superpositions: bool,

    #[serde(default)]
    pub(crate) allow_metadata_only_publications: bool,

    #[serde(default)]
    pub(crate) required_approvals: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Publication {
    pub(crate) id: String,
    pub(crate) snap_id: String,
    pub(crate) scope: String,
    pub(crate) gate: String,
    pub(crate) publisher: String,

    #[serde(default)]
    pub(crate) publisher_user_id: Option<String>,
    pub(crate) created_at: String,

    #[serde(default)]
    pub(crate) resolution: Option<PublicationResolution>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct PublicationResolution {
    pub(crate) bundle_id: String,
    pub(crate) root_manifest: String,
    pub(crate) resolved_root_manifest: String,
    pub(crate) created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Bundle {
    pub(crate) id: String,
    pub(crate) scope: String,
    pub(crate) gate: String,
    pub(crate) root_manifest: String,
    pub(crate) input_publications: Vec<String>,
    pub(crate) created_by: String,

    #[serde(default)]
    pub(crate) created_by_user_id: Option<String>,
    pub(crate) created_at: String,

    pub(crate) promotable: bool,
    pub(crate) reasons: Vec<String>,

    #[serde(default)]
    pub(crate) approvals: Vec<String>,

    #[serde(default)]
    pub(crate) approval_user_ids: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Promotion {
    pub(crate) id: String,
    pub(crate) bundle_id: String,
    pub(crate) scope: String,
    pub(crate) from_gate: String,
    pub(crate) to_gate: String,
    pub(crate) promoted_by: String,

    #[serde(default)]
    pub(crate) promoted_by_user_id: Option<String>,
    pub(crate) promoted_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Release {
    pub(crate) id: String,
    pub(crate) channel: String,
    pub(crate) bundle_id: String,
    pub(crate) scope: String,
    pub(crate) gate: String,

    pub(crate) released_by: String,

    #[serde(default)]
    pub(crate) released_by_user_id: Option<String>,

    pub(crate) released_at: String,

    #[serde(default)]
    pub(crate) notes: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Lane {
    pub(crate) id: String,
    pub(crate) members: HashSet<String>,

    #[serde(default)]
    pub(crate) member_user_ids: HashSet<String>,

    #[serde(default)]
    pub(crate) heads: HashMap<String, LaneHead>,

    // Retention roots for unpublished collaboration. We keep a bounded history of head
    // updates so the server can GC aggressively without losing recent WIP context.
    #[serde(default)]
    pub(crate) head_history: HashMap<String, Vec<LaneHead>>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct LaneHead {
    pub(crate) snap_id: String,
    pub(crate) updated_at: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) client_id: Option<String>,
}

pub(crate) const LANE_HEAD_HISTORY_KEEP_LAST: usize = 5;

fn default_true() -> bool {
    true
}
