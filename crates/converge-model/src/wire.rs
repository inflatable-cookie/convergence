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

/// One object in a transfer batch (doc 16 §1c). Kind is the objects-route
/// segment: "blobs" | "manifests" | "recipes".
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectFrame {
    pub kind: String,
    pub id: ObjectId,
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
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
    /// The full snap record travels with the publish (g02.007 batch 7.4):
    /// the server verifies its identity and stores it, so provenance links
    /// into lineage without a separate sync.
    pub snap: crate::snap::SnapRecord,
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
    /// The published snap's parents — provenance links into lineage.
    #[serde(default)]
    pub snap_parents: Vec<String>,
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

/// Triage report (g02.007 batch 7.3): what needs the caller's attention.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InboxReport {
    pub lanes: Vec<InboxLane>,
    pub publications: Vec<InboxPublication>,
    pub bundles: Vec<InboxBundle>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InboxLane {
    pub lane_id: String,
    pub head_snap_id: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InboxPublication {
    pub gate_id: String,
    pub publication_id: String,
    pub lane_id: String,
    pub publisher: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InboxBundle {
    pub bundle_id: String,
    pub gate_id: String,
    /// "resolve" (superposed) or "approve" (short of approvals).
    pub recommendation: String,
    pub approvals: u32,
    pub required_approvals: u32,
}

/// Server-side retention policy, stored per repo in the control plane.
/// Pure evaluation lives server-side; GC (g02.008 batch 8.3) consumes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Keep the newest N releases per channel (None = keep all).
    #[serde(default)]
    pub keep_releases_per_channel: Option<u32>,
    /// Keep the newest N bundles per gate (None = keep all).
    #[serde(default)]
    pub keep_bundles_per_gate: Option<u32>,
    /// Drop consumed publications older than N days (None = keep all).
    #[serde(default)]
    pub keep_publication_days: Option<u32>,
}

/// A release: a bundle designated for consumption on a named channel.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseRecord {
    pub channel: String,
    pub repo_id: String,
    pub scope_id: String,
    pub bundle_id: String,
    pub released_by: String,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseRequest {
    pub repo_id: String,
    pub scope_id: String,
    pub channel: String,
    pub notes: Option<String>,
}

/// Provenance replay result (g02.008 batch 8.4): the audit feature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerifyReport {
    pub verified: bool,
    pub bundle_id: String,
    pub recorded_root: Option<ObjectId>,
    pub recomputed_root: Option<ObjectId>,
    pub recomputed_id: String,
    pub detail: String,
}

/// A bundle plus its input publications — readable provenance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BundleProvenance {
    pub bundle: BundleRecord,
    pub inputs: Vec<PublicationRecord>,
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
    /// Whether bundles produced by this gate may be released to channels.
    #[serde(default)]
    pub may_release: bool,
}

fn default_strategy() -> String {
    "whole-file".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateGraph {
    pub gates: Vec<GateNode>,
}
