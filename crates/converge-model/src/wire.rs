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

/// Register a scope in a repo (g02.014 batch 14.3).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateScopeRequest {
    pub scope_id: String,
}

/// Create a repo, its default scope, and a starting gate (g02.016 batch
/// 16.3). Server admins only — this is the operation that exists before
/// any repo does.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateRepoRequest {
    pub repo_id: String,
}

/// Add a teammate to a repo: upsert the user, grant capabilities, and
/// optionally issue a token they can log in with (g02.016 batch 16.3).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMemberRequest {
    pub subject: String,
    pub capabilities: Vec<String>,
    /// Scope pattern the grants apply to; `*` for the whole repo.
    pub scope_pattern: String,
    /// Mint a token for this subject and return it once.
    pub issue_token: bool,
}

/// Register a public key for the calling subject (g02.019 batch 19.1).
///
/// The subject is taken from the token, never from the body: a caller
/// must not be able to register a key *as* someone else, which would
/// let them receive secrets meant for that person.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterKeyRequest {
    /// age recipient string (`age1...`).
    pub public_key: String,
    /// Free-text hint, usually the machine it was made on.
    pub label: String,
}

/// A registered public key. Public by definition — this record carries
/// nothing that needs protecting.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicKeyRecord {
    pub key_id: String,
    pub subject: String,
    pub public_key: String,
    pub label: String,
    pub created_at: String,
}

/// An encrypted secret as the server holds it (g02.019, doc 19 §3).
///
/// `ciphertext` is an armored age file. The server stores and returns it
/// byte-exact and has no code path that parses it — armored rather than
/// binary so a database row is visibly an age file and obviously not
/// plaintext.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRecord {
    pub name: String,
    pub owner: String,
    /// Key ids that can decrypt. One entry until `g02.020` adds sharing.
    pub recipients: Vec<String>,
    pub ciphertext: String,
    pub version: u64,
    pub updated_at: String,
    pub updated_by: String,
}

/// A secret without its ciphertext, for listings.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretSummary {
    pub name: String,
    pub owner: String,
    pub recipients: Vec<String>,
    pub version: u64,
    pub updated_at: String,
    pub updated_by: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetSecretRequest {
    pub ciphertext: String,
    pub recipients: Vec<String>,
    /// Version the writer believes it is replacing; `0` creates. A
    /// mismatch is refused rather than overwritten, so two people
    /// rotating the same credential cannot silently lose one of them.
    pub expected_version: u64,
}

/// One member's standing in a repo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemberRecord {
    pub subject: String,
    /// (capability, scope_pattern) pairs, ordered.
    pub grants: Vec<(String, String)>,
}

/// Result of adding a member. `token` is present exactly once — when it
/// was just issued — because the server keeps only its hash.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemberAdded {
    pub subject: String,
    pub granted: Vec<String>,
    pub token: Option<String>,
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
    /// A section hit its cap and was cut (g02.015 batch 15.2). The report
    /// stays bounded on a large repo; this says so rather than passing a
    /// partial list off as the whole picture.
    #[serde(default)]
    pub truncated: bool,
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
    /// Keep the newest N events per repo (None = keep all). Pruning
    /// raises the repo's event floor; cursors below it get `gap: true`.
    #[serde(default)]
    pub keep_events: Option<u32>,
}

/// One page of a listing (g02.015 batch 15.2). `next_cursor` is the
/// value to pass as `after` for the following page; `None` means the
/// listing is exhausted. Pages are capped server-side, so an old client
/// that sends no `limit` still cannot pull an unbounded response.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// One page of the event feed plus the cursor honesty a client needs
/// (g02.014 batch 14.4). Events are hints, so a gap costs freshness,
/// not correctness — but the client must be told it has one.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EventPage {
    pub events: Vec<EventRecord>,
    /// Highest pruned seq for this repo; events at or below are gone.
    pub floor: u64,
    /// The requested cursor sat below `floor`: pruned events were never
    /// delivered to this caller. Reconcile via inbox/status.
    pub gap: bool,
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

/// A convergence event (doc 14 §5b): a hint that something changed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventRecord {
    pub seq: u64,
    pub repo_id: String,
    /// "bundle" | "lane" | "release"
    pub kind: String,
    /// bundle id, lane id, or channel name.
    pub subject_id: String,
    pub created_at: String,
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
