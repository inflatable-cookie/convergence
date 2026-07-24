use anyhow::Result;

use converge_model::{
    BundleStatus, GateGraph, LaneHead, LaneRecord, ObjectId, PublicationRecord, ReleaseRecord,
    RetentionPolicy, SnapRecord,
};

/// Content-addressed object storage (blobs, manifests, recipes). Embedded
/// impl is sharded local FS; external impls (S3) arrive later (arch 14).
pub trait ObjectStore: Send + Sync {
    fn put(&self, kind: ObjectKind, bytes: &[u8]) -> Result<ObjectId>;
    fn put_bytes(&self, kind: ObjectKind, id: &ObjectId, bytes: &[u8]) -> Result<()>;
    fn get(&self, kind: ObjectKind, id: &ObjectId) -> Result<Vec<u8>>;
    fn has(&self, kind: ObjectKind, id: &ObjectId) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    Blob,
    Manifest,
    Recipe,
}

impl ObjectKind {
    pub fn dir(&self) -> &'static str {
        match self {
            ObjectKind::Blob => "blobs",
            ObjectKind::Manifest => "manifests",
            ObjectKind::Recipe => "recipes",
        }
    }
}

/// A bundle as the server stores it: the wire record plus policy state.
#[derive(Clone, Debug)]
pub struct StoredBundle {
    pub bundle_id: String,
    pub repo_id: String,
    pub scope_id: String,
    pub gate_id: String,
    pub inputs: Vec<String>,
    pub root_manifest: Option<ObjectId>,
    /// W: bundle whose root this build folded onto (doc 17 §3).
    pub base_bundle_id: Option<String>,
    /// (first_seq, last_seq) of the consumed publication window.
    pub window: (u64, u64),
    pub strategy: String,
    pub status: BundleStatus,
    pub created_at: String,
}

/// Per-(repo, scope, gate) window state (doc 17 §3).
#[derive(Clone, Debug, Default)]
pub struct PartitionState {
    /// Highest publication seq consumed by the last promoted bundle.
    pub window_floor: u64,
    /// The last promoted bundle (W for the next build).
    pub base_bundle_id: Option<String>,
}

/// Control-plane + partition metadata. Embedded impl is SQLite; every
/// mutation is a scoped transaction (arch 14: no whole-repo rewrites).
pub trait MetadataStore: Send + Sync {
    // control plane
    fn upsert_user(&self, handle: &str) -> Result<()>;
    fn add_grant(
        &self,
        subject: &str,
        repo_id: &str,
        scope_pattern: &str,
        capability: &str,
    ) -> Result<()>;
    fn has_grant(
        &self,
        subject: &str,
        repo_id: &str,
        scope_id: &str,
        capability: &str,
    ) -> Result<bool>;
    fn create_repo(&self, repo_id: &str) -> Result<()>;
    fn repo_exists(&self, repo_id: &str) -> Result<bool>;
    fn set_gate_graph(&self, repo_id: &str, graph: &GateGraph) -> Result<()>;
    fn get_gate_graph(&self, repo_id: &str) -> Result<GateGraph>;

    // lanes (g02.007)
    fn create_lane(&self, lane: &LaneRecord) -> Result<()>;
    fn get_lane(&self, repo_id: &str, lane_id: &str) -> Result<Option<LaneRecord>>;
    fn list_lanes(&self, repo_id: &str) -> Result<Vec<LaneRecord>>;
    fn add_lane_member(&self, repo_id: &str, lane_id: &str, member: &str) -> Result<()>;

    // unpublished sync (g02.007 batch 7.2)
    fn put_snap_record(&self, repo_id: &str, snap: &SnapRecord) -> Result<()>;
    fn get_snap_record(&self, repo_id: &str, snap_id: &str) -> Result<Option<SnapRecord>>;
    fn set_lane_head(&self, repo_id: &str, head: &LaneHead) -> Result<()>;
    fn get_lane_head(&self, repo_id: &str, lane_id: &str) -> Result<Option<LaneHead>>;

    // partition state (repo, scope, gate)
    fn add_publication(&self, publication: &PublicationRecord) -> Result<()>;
    fn get_publication(&self, publication_id: &str) -> Result<Option<PublicationRecord>>;
    /// Publications with seq > `after_seq`, ordered, paired with their seq.
    fn list_publications_after(
        &self,
        repo_id: &str,
        scope_id: &str,
        gate_id: &str,
        after_seq: u64,
    ) -> Result<Vec<(u64, PublicationRecord)>>;
    fn get_partition_state(
        &self,
        repo_id: &str,
        scope_id: &str,
        gate_id: &str,
    ) -> Result<PartitionState>;
    fn set_partition_state(
        &self,
        repo_id: &str,
        scope_id: &str,
        gate_id: &str,
        state: &PartitionState,
    ) -> Result<()>;
    fn put_bundle(&self, bundle: &StoredBundle) -> Result<()>;
    fn get_bundle(&self, bundle_id: &str) -> Result<StoredBundle>;
    fn list_bundles(&self, repo_id: &str, scope_id: &str) -> Result<Vec<StoredBundle>>;
    fn add_approval(&self, bundle_id: &str, approver: &str) -> Result<()>;
    fn count_approvals(&self, bundle_id: &str) -> Result<u32>;
    // retention (g02.008)
    fn set_retention(&self, repo_id: &str, policy: &RetentionPolicy) -> Result<()>;
    fn get_retention(&self, repo_id: &str) -> Result<RetentionPolicy>;

    // releases (g02.008)
    fn add_release(&self, release: &ReleaseRecord) -> Result<()>;
    fn list_releases(&self, repo_id: &str) -> Result<Vec<ReleaseRecord>>;
    fn get_channel_head(&self, repo_id: &str, channel: &str) -> Result<Option<ReleaseRecord>>;

    fn record_promotion(
        &self,
        bundle_id: &str,
        from_gate: &str,
        to_gate: &str,
        at: &str,
    ) -> Result<()>;
    fn list_promotions(&self, bundle_id: &str) -> Result<Vec<(String, String, String)>>;
}
