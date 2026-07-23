use anyhow::Result;

use converge_model::{BundleStatus, GateGraph, ObjectId, PublicationRecord};

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
    pub status: BundleStatus,
    pub created_at: String,
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

    // partition state (repo, scope, gate)
    fn add_publication(&self, publication: &PublicationRecord) -> Result<()>;
    fn list_publications(
        &self,
        repo_id: &str,
        scope_id: &str,
        gate_id: &str,
    ) -> Result<Vec<PublicationRecord>>;
    fn put_bundle(&self, bundle: &StoredBundle) -> Result<()>;
    fn get_bundle(&self, bundle_id: &str) -> Result<StoredBundle>;
    fn add_approval(&self, bundle_id: &str, approver: &str) -> Result<()>;
    fn count_approvals(&self, bundle_id: &str) -> Result<u32>;
    fn record_promotion(
        &self,
        bundle_id: &str,
        from_gate: &str,
        to_gate: &str,
        at: &str,
    ) -> Result<()>;
    fn list_promotions(&self, bundle_id: &str) -> Result<Vec<(String, String, String)>>;
}
