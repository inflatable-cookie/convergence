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
    #[serde(default)]
    pub base_bundle_id: Option<String>,
    /// `None` -> the publisher's auto-provisioned personal lane.
    #[serde(default)]
    pub lane_id: Option<String>,
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
    /// The bundle the publisher last saw for the target (doc 17 §2).
    #[serde(default)]
    pub base_bundle_id: Option<String>,
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
    /// W: the promoted bundle this build folded onto (doc 17 §3).
    #[serde(default)]
    pub base_bundle_id: Option<String>,
    /// (first_seq, last_seq) of the publication window.
    #[serde(default)]
    pub window: (u64, u64),
    /// Coalesce strategy recorded in provenance (doc 17 §4).
    #[serde(default)]
    pub strategy: String,
    pub status: BundleStatus,
    pub created_at: String,
}

/// A registered lane (g02.007): ownership and visibility for the
/// breadth/visibility partition. Publications may only name registered
/// lanes; `personal/<subject>` lanes auto-provision on first use.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaneRecord {
    pub lane_id: String,
    pub repo_id: String,
    pub owner: String,
    pub members: Vec<String>,
    /// "private" (owner + members) or "repo" (visible to repo readers).
    pub visibility: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateLaneRequest {
    pub lane_id: String,
    pub visibility: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddLaneMemberRequest {
    pub member: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaneHead {
    pub lane_id: String,
    pub snap_id: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetLaneHeadRequest {
    /// `None` -> the caller's personal lane (auto-provisioned).
    #[serde(default)]
    pub lane_id: Option<String>,
    pub snap_id: String,
    /// Allow a non-fast-forward head move.
    #[serde(default)]
    pub force: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateNode {
    pub gate_id: String,
    pub name: String,
    pub upstreams: Vec<String>,
    pub required_approvals: u32,
    /// Coalesce strategy (doc 17 §4): "whole-file" (default) or
    /// "text-line-merge".
    #[serde(default = "default_strategy")]
    pub strategy: String,
}

fn default_strategy() -> String {
    "whole-file".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateGraph {
    pub gates: Vec<GateNode>,
}
