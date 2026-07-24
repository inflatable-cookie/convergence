use anyhow::{Result, bail};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use converge_model::{
    BundleStatus, InboxBundle, InboxLane, InboxPublication, InboxReport, LaneHead, LaneRecord,
    ObjectId, PublicationRecord, ReleaseRecord, SnapRecord, VerifyReport,
};

use crate::authz::{AuthzContext, Capability};
use crate::merge::{MergeInput, merge_window};
use crate::storage::{MetadataStore, ObjectStore, PartitionState, StoredBundle};

/// The convergence engine: publish intake, deterministic bundle builds, and
/// policy-checked promotion. Every method takes an [`AuthzContext`] minted by
/// `authz::authorize` — there is no unauthorized path in by construction.
pub struct Engine<'a> {
    pub meta: &'a dyn MetadataStore,
    pub objects: &'a dyn ObjectStore,
}

pub struct PublishInput {
    pub gate_id: String,
    /// Full snap record: identity-verified and stored on publish.
    pub snap: SnapRecord,
    /// The bundle the publisher last saw for this target (doc 17 §2).
    pub base_bundle_id: Option<String>,
    /// `None` -> the publisher's auto-provisioned personal lane.
    pub lane_id: Option<String>,
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
        if !self.objects.has(
            crate::storage::ObjectKind::Manifest,
            &input.snap.root_manifest,
        ) {
            bail!(
                "root manifest {} not uploaded",
                input.snap.root_manifest.as_str()
            );
        }
        // Identity-verify and persist the snap record (provenance links
        // into lineage; rejects tampered records).
        self.upload_snap_record(&authz, &input.snap)?;
        if let Some(base_id) = &input.base_bundle_id {
            let base = self
                .meta
                .get_bundle(base_id)
                .map_err(|_| anyhow::anyhow!("declared base bundle {base_id} is unknown"))?;
            if base.repo_id != authz.repo_id()
                || base.scope_id != authz.scope_id()
                || base.gate_id != input.gate_id
            {
                bail!("declared base bundle {base_id} belongs to another partition");
            }
        }

        // Lane resolution (g02.007): publications name registered lanes
        // only. No lane -> the publisher's personal lane, auto-provisioned.
        let lane_id = self.resolve_writable_lane(&authz, &input.lane_id)?;

        let created_at = now();
        let publication_id = {
            let mut hasher = blake3::Hasher::new();
            hasher.update(authz.repo_id().as_bytes());
            hasher.update(authz.scope_id().as_bytes());
            hasher.update(input.gate_id.as_bytes());
            hasher.update(input.snap.id.as_bytes());
            hasher.update(authz.subject().as_bytes());
            hasher.update(created_at.as_bytes());
            hasher.finalize().to_hex().to_string()
        };
        self.meta.add_publication(&PublicationRecord {
            publication_id,
            snap_id: input.snap.id.clone(),
            root_manifest: input.snap.root_manifest.clone(),
            base_bundle_id: input.base_bundle_id.clone(),
            snap_parents: input.snap.parents.clone(),
            repo_id: authz.repo_id().to_string(),
            scope_id: authz.scope_id().to_string(),
            target_gate_id: input.gate_id.clone(),
            lane_id,
            publisher: authz.subject().to_string(),
            created_at,
            notes: input.notes.clone(),
        })?;

        self.build_bundle(&authz, &input.gate_id)
    }

    /// Deterministic bundle build over the partition's current window
    /// (doc 17 §3): fold the window's publications onto W.
    fn build_bundle(&self, authz: &AuthzContext, gate_id: &str) -> Result<StoredBundle> {
        let partition =
            self.meta
                .get_partition_state(authz.repo_id(), authz.scope_id(), gate_id)?;
        let window = self.meta.list_publications_after(
            authz.repo_id(),
            authz.scope_id(),
            gate_id,
            partition.window_floor,
        )?;
        assert!(!window.is_empty(), "publish just added one");

        let graph = self.meta.get_gate_graph(authz.repo_id())?;
        let strategy = graph
            .gates
            .iter()
            .find(|g| g.gate_id == gate_id)
            .map(|g| g.strategy.clone())
            .unwrap_or_else(|| "whole-file".to_string());

        let w_root = match &partition.base_bundle_id {
            Some(id) => self.meta.get_bundle(id)?.root_manifest,
            None => None,
        };

        let inputs: Result<Vec<MergeInput>> = window
            .iter()
            .map(|(_, p)| {
                let base = match &p.base_bundle_id {
                    Some(id) => self.meta.get_bundle(id)?.root_manifest,
                    None => None,
                };
                Ok(MergeInput {
                    lane: p.lane_id.clone(),
                    base,
                    tree: p.root_manifest.clone(),
                })
            })
            .collect();
        let input_ids: Vec<String> = window
            .iter()
            .map(|(_, p)| p.publication_id.clone())
            .collect();
        let window_range = (
            window.first().map(|(s, _)| *s).unwrap_or(0),
            window.last().map(|(s, _)| *s).unwrap_or(0),
        );

        let hash_id = |root: Option<&ObjectId>| {
            bundle_hash(gate_id, w_root.as_ref(), &input_ids, &strategy, root)
        };

        let bundle = match inputs
            .and_then(|inputs| merge_window(self.objects, w_root.as_ref(), &inputs, &strategy))
        {
            Ok(root) => {
                let has_superpositions = self.manifest_has_superpositions(&root)?;
                StoredBundle {
                    bundle_id: hash_id(Some(&root)),
                    repo_id: authz.repo_id().to_string(),
                    scope_id: authz.scope_id().to_string(),
                    gate_id: gate_id.to_string(),
                    inputs: input_ids,
                    root_manifest: Some(root),
                    base_bundle_id: partition.base_bundle_id.clone(),
                    window: window_range,
                    strategy,
                    status: BundleStatus::Ready {
                        promotable: !has_superpositions,
                    },
                    created_at: now(),
                }
            }
            Err(err) => StoredBundle {
                bundle_id: hash_id(None),
                repo_id: authz.repo_id().to_string(),
                scope_id: authz.scope_id().to_string(),
                gate_id: gate_id.to_string(),
                inputs: input_ids,
                root_manifest: None,
                base_bundle_id: partition.base_bundle_id.clone(),
                window: window_range,
                strategy,
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
        let manifest: converge_model::Manifest = converge_model::encoding::decode_manifest(&bytes)?;
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

    /// Resolve `lane_id` to a registered lane the subject may write:
    /// `None` auto-provisions the personal lane; named lanes require
    /// owner/membership (shared with publish and lane-head pushes).
    fn resolve_writable_lane(
        &self,
        authz: &AuthzContext,
        lane_id: &Option<String>,
    ) -> Result<String> {
        match lane_id {
            Some(lane_id) => {
                let lane = self
                    .meta
                    .get_lane(authz.repo_id(), lane_id)?
                    .ok_or_else(|| anyhow::anyhow!("lane {lane_id} is not registered"))?;
                if lane.owner != authz.subject()
                    && !lane.members.contains(&authz.subject().to_string())
                {
                    bail!(
                        "{} is not an owner or member of lane {lane_id}",
                        authz.subject()
                    );
                }
                Ok(lane_id.clone())
            }
            None => {
                let personal = format!("personal/{}", authz.subject());
                if self.meta.get_lane(authz.repo_id(), &personal)?.is_none() {
                    self.meta.create_lane(&LaneRecord {
                        lane_id: personal.clone(),
                        repo_id: authz.repo_id().to_string(),
                        owner: authz.subject().to_string(),
                        members: Vec::new(),
                        visibility: "private".to_string(),
                        created_at: now(),
                    })?;
                }
                Ok(personal)
            }
        }
    }

    /// Push a lane head (unpublished sync). Snap records for the new head's
    /// lineage must already be uploaded; the move must fast-forward from
    /// the current head unless forced.
    pub fn set_lane_head(
        &self,
        authz: AuthzContext,
        lane_id: Option<String>,
        snap_id: &str,
        force: bool,
    ) -> Result<LaneHead> {
        require(&authz, Capability::Publish)?;
        let lane_id = self.resolve_writable_lane(&authz, &lane_id)?;

        if self
            .meta
            .get_snap_record(authz.repo_id(), snap_id)?
            .is_none()
        {
            bail!("snap {snap_id} has not been uploaded");
        }
        if let Some(current) = self.meta.get_lane_head(authz.repo_id(), &lane_id)?
            && !force
            && !self.is_ancestor(authz.repo_id(), &current.snap_id, snap_id)?
        {
            bail!(
                "non-fast-forward: {} is not an ancestor of {snap_id} (use force)",
                current.snap_id
            );
        }
        let head = LaneHead {
            lane_id,
            snap_id: snap_id.to_string(),
            updated_at: now(),
        };
        self.meta.set_lane_head(authz.repo_id(), &head)?;
        Ok(head)
    }

    /// Is `ancestor` reachable from `descendant` via uploaded snap records?
    fn is_ancestor(&self, repo_id: &str, ancestor: &str, descendant: &str) -> Result<bool> {
        let mut stack = vec![descendant.to_string()];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if id == ancestor {
                return Ok(true);
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(record) = self.meta.get_snap_record(repo_id, &id)? {
                stack.extend(record.parents.iter().cloned());
            }
        }
        Ok(false)
    }

    /// Read access to a lane: owner/members always; repo-visible lanes for
    /// any subject holding the read capability the caller already proved.
    pub fn check_lane_readable(&self, authz: &AuthzContext, lane_id: &str) -> Result<()> {
        let lane = self
            .meta
            .get_lane(authz.repo_id(), lane_id)?
            .ok_or_else(|| anyhow::anyhow!("lane {lane_id} is not registered"))?;
        let subject = authz.subject().to_string();
        if lane.visibility == "repo" || lane.owner == subject || lane.members.contains(&subject) {
            Ok(())
        } else {
            bail!("lane {lane_id} is private to its owner and members")
        }
    }

    pub fn upload_snap_record(&self, authz: &AuthzContext, snap: &SnapRecord) -> Result<()> {
        // Verify declared identity before storing (mirrors object stores'
        // verify-on-write).
        let expected = converge_model::compute_snap_id(
            &snap.root_manifest,
            &snap.parents,
            snap.derived_from_bundle.as_deref(),
        );
        if expected != snap.id {
            bail!("snap record identity mismatch (expected {expected})");
        }
        self.meta.put_snap_record(authz.repo_id(), snap)
    }

    /// Triage report: readable lane heads (newer than `since`), the
    /// scope's current-window publications, and bundles awaiting action.
    pub fn inbox(&self, authz: &AuthzContext, since: Option<&str>) -> Result<InboxReport> {
        require(authz, Capability::Read)?;
        let mut report = InboxReport::default();

        for lane in self.meta.list_lanes(authz.repo_id())? {
            if self.check_lane_readable(authz, &lane.lane_id).is_err() {
                continue;
            }
            if let Some(head) = self.meta.get_lane_head(authz.repo_id(), &lane.lane_id)? {
                if since.is_some_and(|s| head.updated_at.as_str() <= s) {
                    continue;
                }
                report.lanes.push(InboxLane {
                    lane_id: lane.lane_id,
                    head_snap_id: head.snap_id,
                    updated_at: head.updated_at,
                });
            }
        }

        let graph = self.meta.get_gate_graph(authz.repo_id())?;
        for gate in &graph.gates {
            let partition =
                self.meta
                    .get_partition_state(authz.repo_id(), authz.scope_id(), &gate.gate_id)?;
            for (_, publication) in self.meta.list_publications_after(
                authz.repo_id(),
                authz.scope_id(),
                &gate.gate_id,
                partition.window_floor,
            )? {
                report.publications.push(InboxPublication {
                    gate_id: gate.gate_id.clone(),
                    publication_id: publication.publication_id,
                    lane_id: publication.lane_id,
                    publisher: publication.publisher,
                    created_at: publication.created_at,
                });
            }
        }

        // Latest bundle per gate that still needs someone.
        let mut latest: std::collections::BTreeMap<String, crate::storage::StoredBundle> =
            Default::default();
        for bundle in self.meta.list_bundles(authz.repo_id(), authz.scope_id())? {
            latest.insert(bundle.gate_id.clone(), bundle);
        }
        for (gate_id, bundle) in latest {
            let required = graph
                .gates
                .iter()
                .find(|g| g.gate_id == gate_id)
                .map(|g| g.required_approvals)
                .unwrap_or(0);
            let approvals = self.meta.count_approvals(&bundle.bundle_id)?;
            let recommendation = match bundle.status {
                BundleStatus::Ready { promotable: false } => "resolve",
                BundleStatus::Ready { promotable: true } if approvals < required => "approve",
                _ => continue,
            };
            report.bundles.push(InboxBundle {
                bundle_id: bundle.bundle_id,
                gate_id,
                recommendation: recommendation.to_string(),
                approvals,
                required_approvals: required,
            });
        }
        Ok(report)
    }

    pub fn approve(&self, authz: AuthzContext, bundle_id: &str) -> Result<u32> {
        require(&authz, Capability::Approve)?;
        let bundle = self.meta.get_bundle(bundle_id)?;
        ensure_partition(&authz, &bundle)?;
        self.meta.add_approval(bundle_id, authz.subject())?;
        self.meta.count_approvals(bundle_id)
    }

    /// Provenance replay (vision: determinism as a product feature):
    /// re-run the recorded merge and prove the bundle's identity.
    pub fn verify(&self, bundle_id: &str) -> Result<VerifyReport> {
        let bundle = self.meta.get_bundle(bundle_id)?;
        let w_root = match &bundle.base_bundle_id {
            Some(id) => self.meta.get_bundle(id)?.root_manifest,
            None => None,
        };
        let mut inputs = Vec::new();
        for publication_id in &bundle.inputs {
            let publication = self.meta.get_publication(publication_id)?.ok_or_else(|| {
                anyhow::anyhow!("provenance incomplete: publication {publication_id} missing")
            })?;
            let base = match &publication.base_bundle_id {
                Some(id) => self.meta.get_bundle(id)?.root_manifest,
                None => None,
            };
            inputs.push(MergeInput {
                lane: publication.lane_id,
                base,
                tree: publication.root_manifest,
            });
        }
        let recomputed_root =
            merge_window(self.objects, w_root.as_ref(), &inputs, &bundle.strategy)?;
        let recomputed_id = bundle_hash(
            &bundle.gate_id,
            w_root.as_ref(),
            &bundle.inputs,
            &bundle.strategy,
            Some(&recomputed_root),
        );
        let root_matches = bundle.root_manifest.as_ref() == Some(&recomputed_root);
        let id_matches = recomputed_id == bundle.bundle_id;
        Ok(VerifyReport {
            verified: root_matches && id_matches,
            bundle_id: bundle.bundle_id.clone(),
            recorded_root: bundle.root_manifest.clone(),
            recomputed_root: Some(recomputed_root),
            recomputed_id,
            detail: if root_matches && id_matches {
                "replayed merge reproduces the recorded bundle".to_string()
            } else if !root_matches {
                "recomputed root manifest differs from the recorded one".to_string()
            } else {
                "recomputed bundle id differs from the recorded one".to_string()
            },
        })
    }

    /// The sixth verb: designate a ready, promotable bundle for
    /// consumption on a named channel. Policy: the producing gate must be
    /// marked `may_release` (vision: release is a policy-driven output,
    /// not the terminal gate by definition).
    pub fn release(
        &self,
        authz: AuthzContext,
        bundle_id: &str,
        channel: &str,
        notes: Option<String>,
    ) -> Result<ReleaseRecord> {
        require(&authz, Capability::Release)?;
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
        let gate = graph
            .gates
            .iter()
            .find(|g| g.gate_id == bundle.gate_id)
            .ok_or_else(|| anyhow::anyhow!("unknown producing gate {}", bundle.gate_id))?;
        if !gate.may_release {
            bail!("gate {} may not release", bundle.gate_id);
        }
        let release = ReleaseRecord {
            channel: channel.to_string(),
            repo_id: authz.repo_id().to_string(),
            scope_id: authz.scope_id().to_string(),
            bundle_id: bundle_id.to_string(),
            released_by: authz.subject().to_string(),
            notes,
            created_at: now(),
        };
        self.meta.add_release(&release)?;
        Ok(release)
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
            .record_promotion(bundle_id, &bundle.gate_id, to_gate, &now())?;

        // Promotion advances the window (doc 17 §3): the promoted bundle
        // becomes W and its window's publications leave the pool.
        self.meta.set_partition_state(
            authz.repo_id(),
            authz.scope_id(),
            &bundle.gate_id,
            &PartitionState {
                window_floor: bundle.window.1,
                base_bundle_id: Some(bundle.bundle_id.clone()),
            },
        )
    }
}

/// Deterministic bundle identity (doc 17 §3): hash(gate, W root, ordered
/// input publication ids, strategy, merged root).
pub fn bundle_hash(
    gate_id: &str,
    w_root: Option<&ObjectId>,
    input_ids: &[String],
    strategy: &str,
    root: Option<&ObjectId>,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(gate_id.as_bytes());
    if let Some(w) = w_root {
        hasher.update(w.as_str().as_bytes());
    }
    for id in input_ids {
        hasher.update(id.as_bytes());
    }
    hasher.update(strategy.as_bytes());
    if let Some(root) = root {
        hasher.update(root.as_str().as_bytes());
    }
    hasher.finalize().to_hex().to_string()
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
