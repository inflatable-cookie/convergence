use anyhow::{Result, bail};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use converge_model::{BundleStatus, ObjectId, PublicationRecord};

use crate::authz::{AuthzContext, Capability};
use crate::merge::merge_manifests;
use crate::storage::{MetadataStore, ObjectStore, StoredBundle};

/// The convergence engine: publish intake, deterministic bundle builds, and
/// policy-checked promotion. Every method takes an [`AuthzContext`] minted by
/// `authz::authorize` — there is no unauthorized path in by construction.
pub struct Engine<'a> {
    pub meta: &'a dyn MetadataStore,
    pub objects: &'a dyn ObjectStore,
}

pub struct PublishInput {
    pub gate_id: String,
    pub snap_id: String,
    pub root_manifest: ObjectId,
    pub lane_id: String,
    pub notes: Option<String>,
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("format rfc3339")
}

impl Engine<'_> {
    /// Publish intake + synchronous bundle build for the partition. The
    /// build is deterministic: bundle_id = hash(gate, ordered input ids,
    /// merged root manifest).
    pub fn publish(&self, authz: AuthzContext, input: PublishInput) -> Result<StoredBundle> {
        require(&authz, Capability::Publish)?;
        if !self.meta.repo_exists(authz.repo_id())? {
            bail!("unknown repo {}", authz.repo_id());
        }
        let graph = self.meta.get_gate_graph(authz.repo_id())?;
        if !graph.gates.iter().any(|g| g.gate_id == input.gate_id) {
            bail!("unknown gate {} in repo {}", input.gate_id, authz.repo_id());
        }
        if !self
            .objects
            .has(crate::storage::ObjectKind::Manifest, &input.root_manifest)
        {
            bail!(
                "root manifest {} not uploaded",
                input.root_manifest.as_str()
            );
        }

        let created_at = now();
        let publication_id = {
            let mut hasher = blake3::Hasher::new();
            hasher.update(authz.repo_id().as_bytes());
            hasher.update(authz.scope_id().as_bytes());
            hasher.update(input.gate_id.as_bytes());
            hasher.update(input.snap_id.as_bytes());
            hasher.update(authz.subject().as_bytes());
            hasher.update(created_at.as_bytes());
            hasher.finalize().to_hex().to_string()
        };
        self.meta.add_publication(&PublicationRecord {
            publication_id,
            snap_id: input.snap_id.clone(),
            root_manifest: input.root_manifest.clone(),
            repo_id: authz.repo_id().to_string(),
            scope_id: authz.scope_id().to_string(),
            target_gate_id: input.gate_id.clone(),
            lane_id: input.lane_id.clone(),
            publisher: authz.subject().to_string(),
            created_at,
            notes: input.notes.clone(),
        })?;

        self.build_bundle(&authz, &input.gate_id)
    }

    /// Deterministic bundle build over the partition's ordered publications.
    fn build_bundle(&self, authz: &AuthzContext, gate_id: &str) -> Result<StoredBundle> {
        let publications =
            self.meta
                .list_publications(authz.repo_id(), authz.scope_id(), gate_id)?;
        assert!(!publications.is_empty(), "publish just added one");

        // Snap roots come from the publications; lane is the provenance
        // source shown on superposition variants.
        let inputs: Vec<(String, ObjectId)> = publications
            .iter()
            .map(|p| (p.lane_id.clone(), p.root_manifest.clone()))
            .collect();
        let input_ids: Vec<String> = publications
            .iter()
            .map(|p| p.publication_id.clone())
            .collect();

        let bundle = match merge_manifests(self.objects, &inputs) {
            Ok(root) => {
                let bundle_id = {
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(gate_id.as_bytes());
                    for id in &input_ids {
                        hasher.update(id.as_bytes());
                    }
                    hasher.update(root.as_str().as_bytes());
                    hasher.finalize().to_hex().to_string()
                };
                let has_superpositions = self.manifest_has_superpositions(&root)?;
                StoredBundle {
                    bundle_id,
                    repo_id: authz.repo_id().to_string(),
                    scope_id: authz.scope_id().to_string(),
                    gate_id: gate_id.to_string(),
                    inputs: input_ids,
                    root_manifest: Some(root),
                    status: BundleStatus::Ready {
                        promotable: !has_superpositions,
                    },
                    created_at: now(),
                }
            }
            Err(err) => StoredBundle {
                bundle_id: {
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(gate_id.as_bytes());
                    for id in &input_ids {
                        hasher.update(id.as_bytes());
                    }
                    hasher.finalize().to_hex().to_string()
                },
                repo_id: authz.repo_id().to_string(),
                scope_id: authz.scope_id().to_string(),
                gate_id: gate_id.to_string(),
                inputs: input_ids,
                root_manifest: None,
                status: BundleStatus::Failed {
                    reason: format!("{err:#}"),
                },
                created_at: now(),
            },
        };
        self.meta.put_bundle(&bundle)?;
        Ok(bundle)
    }

    fn manifest_has_superpositions(&self, root: &ObjectId) -> Result<bool> {
        let bytes = self
            .objects
            .get(crate::storage::ObjectKind::Manifest, root)?;
        let manifest: converge_model::Manifest = serde_json::from_slice(&bytes)?;
        for entry in &manifest.entries {
            let nested = match &entry.kind {
                converge_model::ManifestEntryKind::Superposition { .. } => true,
                converge_model::ManifestEntryKind::Dir { manifest } => {
                    self.manifest_has_superpositions(manifest)?
                }
                _ => false,
            };
            if nested {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn approve(&self, authz: AuthzContext, bundle_id: &str) -> Result<u32> {
        require(&authz, Capability::Approve)?;
        let bundle = self.meta.get_bundle(bundle_id)?;
        ensure_partition(&authz, &bundle)?;
        self.meta.add_approval(bundle_id, authz.subject())?;
        self.meta.count_approvals(bundle_id)
    }

    /// Policy-checked promotion (arch 14 §3): target gate must list the
    /// producing gate upstream; the producing gate's required approvals must
    /// be met; the bundle must be ready and promotable.
    pub fn promote(&self, authz: AuthzContext, bundle_id: &str, to_gate: &str) -> Result<()> {
        require(&authz, Capability::Promote)?;
        let bundle = self.meta.get_bundle(bundle_id)?;
        ensure_partition(&authz, &bundle)?;

        match &bundle.status {
            BundleStatus::Ready { promotable: true } => {}
            BundleStatus::Ready { promotable: false } => {
                bail!("bundle {bundle_id} has unresolved superpositions")
            }
            other => bail!("bundle {bundle_id} is not ready: {other:?}"),
        }

        let graph = self.meta.get_gate_graph(authz.repo_id())?;
        let target = graph
            .gates
            .iter()
            .find(|g| g.gate_id == to_gate)
            .ok_or_else(|| anyhow::anyhow!("unknown target gate {to_gate}"))?;
        if !target.upstreams.contains(&bundle.gate_id) {
            bail!(
                "gate {to_gate} does not accept promotions from {}",
                bundle.gate_id
            );
        }
        let producing = graph
            .gates
            .iter()
            .find(|g| g.gate_id == bundle.gate_id)
            .ok_or_else(|| anyhow::anyhow!("unknown producing gate {}", bundle.gate_id))?;
        let approvals = self.meta.count_approvals(bundle_id)?;
        if approvals < producing.required_approvals {
            bail!(
                "bundle {bundle_id} has {approvals} of {} required approvals",
                producing.required_approvals
            );
        }

        self.meta
            .record_promotion(bundle_id, &bundle.gate_id, to_gate, &now())
    }
}

fn require(authz: &AuthzContext, capability: Capability) -> Result<()> {
    if authz.capability() != capability && authz.capability() != Capability::Admin {
        bail!(
            "authz context carries {}, operation needs {}",
            authz.capability().as_str(),
            capability.as_str()
        );
    }
    Ok(())
}

fn ensure_partition(authz: &AuthzContext, bundle: &StoredBundle) -> Result<()> {
    if bundle.repo_id != authz.repo_id() || bundle.scope_id != authz.scope_id() {
        bail!(
            "bundle {} belongs to {}/{}, authz covers {}/{}",
            bundle.bundle_id,
            bundle.repo_id,
            bundle.scope_id,
            authz.repo_id(),
            authz.scope_id()
        );
    }
    Ok(())
}
