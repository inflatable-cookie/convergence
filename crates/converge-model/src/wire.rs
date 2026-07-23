use serde::{Deserialize, Serialize};

use crate::ids::ObjectId;

/// Protocol major version. Servers refuse unknown majors; no pre-1.0
/// compatibility shims (architecture doc 16).
pub const WIRE_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiateRequest {
    pub wire_version: u32,
    pub object_ids: Vec<ObjectId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiateResponse {
    pub missing: Vec<ObjectId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicationRecord {
    pub publication_id: String,
    pub snap_id: String,
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
