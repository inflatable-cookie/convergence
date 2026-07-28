use anyhow::Result;

use crate::authz::Capability;

use converge_model::{
    BundleStatus, EventRecord, GateGraph, LaneHead, LaneRecord, ObjectId, PublicationRecord,
    ReleaseRecord, RetentionPolicy, SnapRecord,
};

/// Content-addressed object storage (blobs, manifests, recipes). Embedded
/// impl is sharded local FS; external impls (S3) arrive later (arch 14).
pub trait ObjectStore: Send + Sync {
    fn put(&self, kind: ObjectKind, bytes: &[u8]) -> Result<ObjectId>;
    fn put_bytes(&self, kind: ObjectKind, id: &ObjectId, bytes: &[u8]) -> Result<()>;
    fn get(&self, kind: ObjectKind, id: &ObjectId) -> Result<Vec<u8>>;
    fn has(&self, kind: ObjectKind, id: &ObjectId) -> bool;
    /// All stored objects of a kind: (id, bytes, mtime). GC sweep input.
    fn list(&self, kind: ObjectKind) -> Result<Vec<(ObjectId, u64, std::time::SystemTime)>>;
    /// Remove one object (GC sweep only).
    fn delete(&self, kind: ObjectKind, id: &ObjectId) -> Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartitionState {
    /// Highest publication seq consumed by the last promoted bundle.
    pub window_floor: u64,
    /// The last promoted bundle (W for the next build).
    pub base_bundle_id: Option<String>,
}

/// One write (or guard) inside an atomic metadata batch (g02.013 batch
/// 13.1, audit H2). Guards abort the whole batch when violated, so a
/// batch composed against stale reads rolls back instead of committing
/// inconsistent partition state.
#[derive(Clone, Debug)]
pub enum MetaOp {
    AddPublication(PublicationRecord),
    PutBundle(StoredBundle),
    SetPartitionState {
        repo_id: String,
        scope_id: String,
        gate_id: String,
        state: PartitionState,
    },
    RecordPromotion {
        bundle_id: String,
        from_gate: String,
        to_gate: String,
        at: String,
    },
    AddEvent {
        repo_id: String,
        kind: String,
        subject_id: String,
        created_at: String,
    },
    /// Fail the batch unless the partition still has this exact state.
    AssertPartitionState {
        repo_id: String,
        scope_id: String,
        gate_id: String,
        expected: PartitionState,
    },
    /// Store an encrypted secret (g02.019). The server never inspects
    /// `ciphertext`.
    PutSecret {
        repo_id: String,
        record: converge_model::SecretRecord,
    },
    /// Fail the batch unless the secret is still at `expected` (0 = must
    /// not exist yet).
    AssertSecretVersion {
        repo_id: String,
        owner: String,
        name: String,
        expected: u64,
    },
    /// Replace a repo's gate graph (batch 26.2).
    SetGateGraph {
        repo_id: String,
        graph: GateGraph,
    },
    /// Fail the batch unless the graph is still the one the caller read.
    ///
    /// Two admins reshaping at once would otherwise be a lost update,
    /// and a gate graph is exactly the kind of thing two people edit
    /// after agreeing to change it — the loser re-reads and sees what
    /// actually happened instead of silently overwriting it.
    AssertGateGraph {
        repo_id: String,
        expected: GateGraph,
    },
    /// Fail the batch unless exactly `expected` publications exist with
    /// seq > `after_seq` (pins the in-memory window and the next seq).
    AssertPublicationCount {
        repo_id: String,
        scope_id: String,
        gate_id: String,
        after_seq: u64,
        expected: u64,
    },
}

/// Raised by `apply_batch` when a guard op fails; the batch rolled
/// back. Callers re-read and rebuild (publish) or surface the conflict
/// (promote).
#[derive(Debug)]
pub struct BatchConflict(pub String);

impl std::fmt::Display for BatchConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "metadata batch guard failed: {}", self.0)
    }
}

impl std::error::Error for BatchConflict {}

/// Control-plane + partition metadata. Embedded impl is SQLite; every
/// mutation is a scoped transaction (arch 14: no whole-repo rewrites).
pub trait MetadataStore: Send + Sync {
    /// Apply every op in one transaction: all writes commit together or
    /// none do. A failed guard rolls back and returns [`BatchConflict`].
    fn apply_batch(&self, ops: &[MetaOp]) -> Result<()>;
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

    // membership + tokens (g02.016 batch 16.3): onboarding a teammate is
    // a runtime operation, so tokens live in the store rather than only
    // in the process's startup flags.
    /// Store a token by hash. The raw token is never persisted: the
    /// server only ever needs to recognise it, and a leaked database
    /// should not hand an attacker working credentials.
    fn create_token(&self, token_hash: &str, subject: &str) -> Result<()>;
    /// Issue a token with its administrable facts (g02.021 batch 21.1).
    fn create_token_record(
        &self,
        token_hash: &str,
        record: &converge_model::TokenRecord,
    ) -> Result<()>;
    /// The token's record, whatever its state. Expiry and revocation are
    /// judged by the caller so the two can be reported differently.
    fn token_by_hash(&self, token_hash: &str) -> Result<Option<converge_model::TokenRecord>>;
    fn subject_for_token_hash(&self, token_hash: &str) -> Result<Option<String>>;
    fn token_count(&self, subject: &str) -> Result<usize>;
    fn list_tokens(&self, repo_id: &str) -> Result<Vec<converge_model::TokenRecord>>;
    /// Record a revocation. Kept rather than deleted: "this token was
    /// revoked, when, by whom and why" is the question an incident asks.
    fn revoke_token(
        &self,
        token_id: &str,
        at: &str,
        by: &str,
        reason: &str,
    ) -> Result<Option<converge_model::TokenRecord>>;
    fn touch_token(&self, token_hash: &str, at: &str) -> Result<()>;
    /// (subject, capability, scope_pattern) rows for one repo, ordered.
    fn list_grants(&self, repo_id: &str) -> Result<Vec<(String, String, String)>>;
    /// Drop every grant a subject holds in one repo (g02.020 batch
    /// 20.2). The user record survives: they may still hold secrets
    /// sealed to their keys, and erasing the subject would make those
    /// unattributable.
    fn remove_grants(&self, repo_id: &str, subject: &str) -> Result<u64>;

    // public keys (g02.019 batch 19.1): the recipients secrets are
    // sealed to. Public data — stored so members can encrypt to each
    // other without an out-of-band exchange.
    fn add_public_key(&self, repo_id: &str, key: &converge_model::PublicKeyRecord) -> Result<()>;
    fn list_public_keys(&self, repo_id: &str) -> Result<Vec<converge_model::PublicKeyRecord>>;

    // encrypted secrets (g02.019 batch 19.2). Writes go through
    // `apply_batch` so a stale version fails the whole batch.
    fn get_secret(
        &self,
        repo_id: &str,
        owner: &str,
        name: &str,
    ) -> Result<Option<converge_model::SecretRecord>>;
    fn list_secrets(&self, repo_id: &str) -> Result<Vec<converge_model::SecretRecord>>;
    fn delete_secret(&self, repo_id: &str, owner: &str, name: &str) -> Result<()>;

    /// Server-wide admin: a grant recorded against the `*` repo. Repo
    /// grants stay exact-match (see `has_grant`) so this cannot widen an
    /// ordinary repo admin into a site admin by accident.
    fn is_site_admin(&self, subject: &str) -> Result<bool> {
        self.has_grant(subject, "*", "*", Capability::Admin.as_str())
    }
    fn list_repos(&self) -> Result<Vec<String>>;
    fn repo_exists(&self, repo_id: &str) -> Result<bool>;
    fn set_gate_graph(&self, repo_id: &str, graph: &GateGraph) -> Result<()>;
    fn get_gate_graph(&self, repo_id: &str) -> Result<GateGraph>;

    /// What lives in each gate of a repo.
    ///
    /// One call rather than three round trips per gate, because the
    /// caller asking is deciding whether a graph change would strand
    /// work and wants a whole picture, not a series of glimpses of a
    /// moving one.
    ///
    /// Open publications are counted above the partition's window floor:
    /// those are the ones a fold still reads, and therefore the ones
    /// that removing a gate would strand (batch 22.4 finding 34).
    fn gate_occupancy(&self, repo_id: &str) -> Result<Vec<converge_model::gates::GateOccupancy>>;

    // scope registry (g02.014 batch 14.3): scopes are declared repo state,
    // so a typo cannot mint a partition and fragment windows.
    fn create_scope(&self, repo_id: &str, scope_id: &str, created_at: &str) -> Result<()>;
    fn list_scopes(&self, repo_id: &str) -> Result<Vec<String>>;
    fn scope_exists(&self, repo_id: &str, scope_id: &str) -> Result<bool>;

    // paged listings (g02.015 batch 15.2): `after` is the last key the
    // caller saw, ordered by a stable key so a cursor cannot skip or
    // repeat under concurrent inserts.
    fn list_scopes_page(
        &self,
        repo_id: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>>;
    fn list_lanes_page(
        &self,
        repo_id: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<LaneRecord>>;
    /// Releases after `after_seq`, oldest first, paired with their seq.
    fn list_releases_page(
        &self,
        repo_id: &str,
        after_seq: Option<u64>,
        limit: usize,
    ) -> Result<Vec<(u64, ReleaseRecord)>>;
    /// The newest bundle per gate in a scope — at most one row per gate,
    /// so the inbox stops scanning every bundle ever built there.
    fn latest_bundles_per_gate(&self, repo_id: &str, scope_id: &str) -> Result<Vec<StoredBundle>>;

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
    fn list_bundles_all_scopes(&self, repo_id: &str) -> Result<Vec<StoredBundle>>;
    /// All partitions of a repo: (scope, gate, window_floor).
    fn list_partitions(&self, repo_id: &str) -> Result<Vec<(String, String, u64)>>;
    fn add_approval(&self, bundle_id: &str, approver: &str) -> Result<()>;
    fn count_approvals(&self, bundle_id: &str) -> Result<u32>;
    // events (g02.010 batch 10.3)
    fn add_event(
        &self,
        repo_id: &str,
        kind: &str,
        subject_id: &str,
        created_at: &str,
    ) -> Result<u64>;
    fn list_events(&self, repo_id: &str, since: u64) -> Result<Vec<EventRecord>>;
    /// Prune all but the newest `keep` events, raising the repo's event
    /// floor to the highest pruned seq (g02.014 batch 14.4). Returns the
    /// number pruned.
    fn prune_events(&self, repo_id: &str, keep: u32) -> Result<u64>;
    /// Highest pruned event seq: cursors at or below it have a gap.
    fn event_floor(&self, repo_id: &str) -> Result<u64>;

    // retention (g02.008)
    fn set_retention(&self, repo_id: &str, policy: &RetentionPolicy) -> Result<()>;
    fn get_retention(&self, repo_id: &str) -> Result<RetentionPolicy>;

    // releases (g02.008)
    fn add_release(&self, release: &ReleaseRecord) -> Result<()>;
    fn list_releases(&self, repo_id: &str) -> Result<Vec<ReleaseRecord>>;
    /// Exact version lookup — yanked or not, because naming a version
    /// exactly is allowed to reach a withdrawn one (g02.028).
    fn get_release(&self, repo_id: &str, version: &str) -> Result<Option<ReleaseRecord>>;

    /// Mark a release withdrawn. Returns false when no such version.
    fn set_release_yanked(&self, repo_id: &str, version: &str, reason: &str) -> Result<bool>;

    // GC metadata drops (g02.008 batch 8.3)
    fn delete_releases_for_bundles(&self, repo_id: &str, bundle_ids: &[String]) -> Result<u64>;
    fn delete_bundles(&self, repo_id: &str, bundle_ids: &[String]) -> Result<u64>;
    fn delete_publications(&self, repo_id: &str, publication_ids: &[String]) -> Result<u64>;

    fn record_promotion(
        &self,
        bundle_id: &str,
        from_gate: &str,
        to_gate: &str,
        at: &str,
    ) -> Result<()>;
    fn list_promotions(&self, bundle_id: &str) -> Result<Vec<(String, String, String)>>;

    // object→repo association (g02.011 batch 11.1): objects are deduped
    // across repos, so repo membership lives here, not in the object store.
    fn associate_object(&self, repo_id: &str, kind: ObjectKind, id: &ObjectId) -> Result<()>;
    fn object_in_repo(&self, repo_id: &str, kind: ObjectKind, id: &ObjectId) -> Result<bool>;
    /// Drop every repo's association for an object (GC sweep only).
    fn remove_object_associations(&self, kind: ObjectKind, id: &ObjectId) -> Result<()>;

    // upload pins (g02.012 batch 12.2): protect an uploaded object from GC
    // until it becomes durably referenced, independent of clock time.
    fn pin_object(&self, repo_id: &str, kind: ObjectKind, id: &ObjectId) -> Result<()>;
    fn unpin_object(&self, repo_id: &str, kind: ObjectKind, id: &ObjectId) -> Result<()>;
    /// Is this object pinned by any repo? (shared store → global check).
    /// Is this object pinned by an upload no older than `cutoff`?
    ///
    /// The cutoff is what makes the pin expire. Asking here rather than
    /// deleting first means a dry run and a real run reach the same
    /// answer without a dry run mutating anything.
    fn is_object_pinned(&self, kind: ObjectKind, id: &ObjectId, cutoff: i64) -> Result<bool>;

    /// Drop pins older than `cutoff` (unix seconds), returning how many.
    ///
    /// A pin protects an object that has been uploaded but is not yet
    /// referenced by anything. It is released when the tree it belongs
    /// to is published — and if that publish never happens, batch 22.4
    /// found the pin simply stayed, forever, with no timestamp on it to
    /// tell a three-second-old upload from a three-month-old abandoned
    /// one. GC would report the object unreachable and decline to sweep
    /// it on every run for the life of the deployment.
    ///
    /// Expiring the pin does not delete anything by itself: the object
    /// falls back to ordinary reachability, so one that turned out to be
    /// referenced survives regardless.
    fn sweep_stale_pins(&self, cutoff: i64) -> Result<u64>;
}

/// Does a grant's `scope_pattern` cover `scope_id`? The accepted syntax is
/// exactly three shapes (arch 14 §4): `*` (every scope in the repo), a
/// literal scope id, or `prefix/*` covering that path segment prefix.
/// Nothing else is a wildcard — `foo*` matches only the literal `foo*`.
pub fn scope_pattern_matches(pattern: &str, scope_id: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return scope_id
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'));
    }
    pattern == scope_id
}

/// Repo-scoped view over the shared object store: every write also records
/// an object→repo association, and `has` answers for this repo only. Read
/// endpoints enforce the association; `negotiate` reports present-but-
/// unassociated objects as missing so an idempotent re-put repairs the row.
pub struct AssociatingObjects<'a> {
    pub inner: &'a dyn ObjectStore,
    pub meta: &'a dyn MetadataStore,
    pub repo_id: String,
}

impl ObjectStore for AssociatingObjects<'_> {
    fn put(&self, kind: ObjectKind, bytes: &[u8]) -> Result<ObjectId> {
        let id = self.inner.put(kind, bytes)?;
        self.meta.associate_object(&self.repo_id, kind, &id)?;
        // Pin until the object is durably referenced (batch 12.2): GC must
        // not reclaim a fresh upload before its publish/set-lane-head lands.
        self.meta.pin_object(&self.repo_id, kind, &id)?;
        Ok(id)
    }

    fn put_bytes(&self, kind: ObjectKind, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        self.inner.put_bytes(kind, id, bytes)?;
        self.meta.associate_object(&self.repo_id, kind, id)?;
        self.meta.pin_object(&self.repo_id, kind, id)
    }

    fn get(&self, kind: ObjectKind, id: &ObjectId) -> Result<Vec<u8>> {
        self.inner.get(kind, id)
    }

    fn has(&self, kind: ObjectKind, id: &ObjectId) -> bool {
        self.inner.has(kind, id)
            && self
                .meta
                .object_in_repo(&self.repo_id, kind, id)
                .unwrap_or(false)
    }

    fn list(&self, kind: ObjectKind) -> Result<Vec<(ObjectId, u64, std::time::SystemTime)>> {
        self.inner.list(kind)
    }

    fn delete(&self, kind: ObjectKind, id: &ObjectId) -> Result<()> {
        self.inner.delete(kind, id)
    }
}

/// Copy-on-write scratch view (g02.011 batch 11.3): reads fall through to
/// the shared store, writes stay in memory and vanish with the value.
/// `verify` replays merges through this so a GET never mutates storage.
pub struct ScratchObjects<'a> {
    inner: &'a dyn ObjectStore,
    scratch: std::sync::Mutex<std::collections::HashMap<(ObjectKind, String), Vec<u8>>>,
}

impl<'a> ScratchObjects<'a> {
    pub fn over(inner: &'a dyn ObjectStore) -> Self {
        Self {
            inner,
            scratch: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl ObjectStore for ScratchObjects<'_> {
    fn put(&self, kind: ObjectKind, bytes: &[u8]) -> Result<ObjectId> {
        let id = ObjectId(blake3::hash(bytes).to_hex().to_string());
        self.put_bytes(kind, &id, bytes)?;
        Ok(id)
    }

    fn put_bytes(&self, kind: ObjectKind, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        let actual = ObjectId(blake3::hash(bytes).to_hex().to_string());
        if actual != *id {
            anyhow::bail!(
                "{} hash mismatch (expected {}, got {})",
                kind.dir(),
                id.as_str(),
                actual.as_str()
            );
        }
        self.scratch
            .lock()
            .expect("scratch lock")
            .insert((kind, id.as_str().to_string()), bytes.to_vec());
        Ok(())
    }

    fn get(&self, kind: ObjectKind, id: &ObjectId) -> Result<Vec<u8>> {
        if let Some(bytes) = self
            .scratch
            .lock()
            .expect("scratch lock")
            .get(&(kind, id.as_str().to_string()))
        {
            return Ok(bytes.clone());
        }
        self.inner.get(kind, id)
    }

    fn has(&self, kind: ObjectKind, id: &ObjectId) -> bool {
        self.scratch
            .lock()
            .expect("scratch lock")
            .contains_key(&(kind, id.as_str().to_string()))
            || self.inner.has(kind, id)
    }

    fn list(&self, kind: ObjectKind) -> Result<Vec<(ObjectId, u64, std::time::SystemTime)>> {
        // Scratch views never feed GC; underlying listing is enough.
        self.inner.list(kind)
    }

    fn delete(&self, kind: ObjectKind, id: &ObjectId) -> Result<()> {
        self.scratch
            .lock()
            .expect("scratch lock")
            .remove(&(kind, id.as_str().to_string()));
        Ok(())
    }
}
