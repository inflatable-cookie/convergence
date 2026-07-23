use serde::{Deserialize, Serialize};

use crate::ids::ObjectId;

/// Protocol major version. Servers refuse unknown majors; no pre-1.0
/// compatibility shims (architecture doc 16).
pub const WIRE_VERSION: u32 = 1;

/// Object-ID sets grouped by kind, used for negotiation in both directions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ObjectSet {
    #[serde(default)]
    pub blobs: Vec<ObjectId>,
    #[serde(default)]
    pub manifests: Vec<ObjectId>,
    #[serde(default)]
    pub recipes: Vec<ObjectId>,
}

impl ObjectSet {
    pub fn is_empty(&self) -> bool {
        self.blobs.is_empty() && self.manifests.is_empty() && self.recipes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.blobs.len() + self.manifests.len() + self.recipes.len()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiateRequest {
    pub wire_version: u32,
    pub objects: ObjectSet,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiateResponse {
    pub missing: ObjectSet,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishRequest {
    pub wire_version: u32,
    pub repo_id: String,
    pub scope_id: String,
    pub gate_id: String,
    pub snap_id: String,
    pub root_manifest: ObjectId,
    pub lane_id: String,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromoteRequest {
    pub repo_id: String,
    pub scope_id: String,
    pub to_gate: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApproveRequest {
    pub repo_id: String,
    pub scope_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicationRecord {
    pub publication_id: String,
    pub snap_id: String,
    pub root_manifest: ObjectId,
    pub repo_id: String,
    pub scope_id: String,
    pub target_gate_id: String,
    pub lane_id: String,
    pub publisher: String,
    pub created_at: String,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleStatus {
    Building,
    Ready { promotable: bool },
    Failed { reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BundleRecord {
    pub bundle_id: String,
    pub produced_by_gate_id: String,
    pub scope_id: String,
    pub inputs: Vec<String>,
    pub root_manifest: Option<ObjectId>,
    pub status: BundleStatus,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaneHead {
    pub lane_id: String,
    pub snap_id: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateNode {
    pub gate_id: String,
    pub name: String,
    pub upstreams: Vec<String>,
    pub required_approvals: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateGraph {
    pub gates: Vec<GateNode>,
}
