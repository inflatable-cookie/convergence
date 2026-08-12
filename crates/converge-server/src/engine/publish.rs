//! The publish pipeline: candidate build, tree pinning, writable-lane resolution.

use anyhow::{Result, bail};

use converge_model::{CandidateStatus, LaneRecord, ObjectId, PublicationRecord};

use crate::storage::StoredCandidate;

use crate::authz::{AuthzContext, Capability};

use super::{candidate_hash, now, require};

use crate::merge::MergeInput;

use crate::storage::{BatchConflict, MetaOp, PartitionState};

use super::{Engine, PublishInput};

impl Engine<'_> {
    /// Publish intake + synchronous candidate build for the partition. The
    /// build is deterministic: candidate_id = hash(gate, ordered input ids,
    /// merged root manifest).
    pub fn publish(&self, authz: AuthzContext, input: PublishInput) -> Result<StoredCandidate> {
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
        if let Some(base_id) = &input.base_candidate_id {
            let base = self
                .meta
                .get_candidate(base_id)
                .map_err(|_| anyhow::anyhow!("declared base candidate {base_id} is unknown"))?;
            if base.repo_id != authz.repo_id()
                || base.scope_id != authz.scope_id()
                || base.gate_id != input.gate_id
            {
                bail!("declared base candidate {base_id} belongs to another partition");
            }
        }

        // Lane resolution (g02.007): publications name registered lanes
        // only. No lane -> the publisher's personal lane, auto-provisioned.
        let lane_id = self.resolve_writable_lane(&authz, &input.lane_id)?;

        // One atomic operation per attempt (batch 13.1, audit H2): read the
        // partition, compute the publication + merged candidate in memory, then
        // commit everything in a single guarded batch. A concurrent publish
        // trips a guard, rolls the batch back, and we rebuild against the
        // fresh window instead of committing a stale one.
        // Rebuilds are cheap (in-memory merge against the fresh window);
        // the cap only guards against pathological livelock.
        const ATTEMPTS: usize = 32;
        for _ in 0..ATTEMPTS {
            let partition =
                self.meta
                    .get_partition_state(authz.repo_id(), authz.scope_id(), &input.gate_id)?;
            let existing = self.meta.list_publications_after(
                authz.repo_id(),
                authz.scope_id(),
                &input.gate_id,
                partition.window_floor,
            )?;
            // Mirrors the backends' floor-aware seq assignment.
            let next_seq = existing
                .last()
                .map(|(seq, _)| *seq)
                .unwrap_or(0)
                .max(partition.window_floor)
                + 1;

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
            let publication = PublicationRecord {
                publication_id,
                snap_id: input.snap.id.clone(),
                root_manifest: input.snap.root_manifest.clone(),
                base_candidate_id: input.base_candidate_id.clone(),
                snap_parents: input.snap.parents.clone(),
                repo_id: authz.repo_id().to_string(),
                scope_id: authz.scope_id().to_string(),
                target_gate_id: input.gate_id.clone(),
                lane_id: lane_id.clone(),
                publisher: authz.subject().to_string(),
                created_at,
                notes: input.notes.clone(),
            };

            let mut window = existing.clone();
            window.push((next_seq, publication.clone()));
            let candidate = self.build_candidate(&authz, &input.gate_id, &partition, &window)?;

            let ops = [
                MetaOp::AssertPartitionState {
                    repo_id: authz.repo_id().to_string(),
                    scope_id: authz.scope_id().to_string(),
                    gate_id: input.gate_id.clone(),
                    expected: partition.clone(),
                },
                MetaOp::AssertPublicationCount {
                    repo_id: authz.repo_id().to_string(),
                    scope_id: authz.scope_id().to_string(),
                    gate_id: input.gate_id.clone(),
                    after_seq: partition.window_floor,
                    expected: existing.len() as u64,
                },
                MetaOp::AddPublication(publication),
                MetaOp::PutCandidate(candidate.clone()),
                // Event hint (doc 14 §5b): candidate state changed.
                MetaOp::AddEvent {
                    repo_id: authz.repo_id().to_string(),
                    kind: "candidate".to_string(),
                    subject_id: candidate.candidate_id.clone(),
                    created_at: now(),
                },
            ];
            match self.meta.apply_batch(&ops) {
                Ok(()) => {
                    // The publication now references the uploaded tree
                    // durably: release its upload pins (batch 12.2).
                    self.unpin_tree(authz.repo_id(), &input.snap.root_manifest)?;
                    return Ok(candidate);
                }
                Err(err) if err.is::<BatchConflict>() => continue,
                Err(err) => return Err(err),
            }
        }
        bail!("publish kept losing to concurrent publishes after {ATTEMPTS} attempts")
    }

    /// Release upload pins for every object reachable from `root` (batch
    /// 12.2). Tolerates missing objects — a partial tree just unpins less.
    pub(crate) fn unpin_tree(&self, repo_id: &str, root: &ObjectId) -> Result<()> {
        let mut manifests = vec![root.clone()];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = manifests.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            self.meta
                .unpin_object(repo_id, crate::storage::ObjectKind::Manifest, &id)?;
            let Ok(bytes) = self.objects.get(crate::storage::ObjectKind::Manifest, &id) else {
                continue;
            };
            let manifest: converge_model::Manifest =
                converge_model::encoding::decode_manifest(&bytes)?;
            for entry in manifest.entries {
                self.unpin_entry(repo_id, entry.kind, &mut manifests)?;
            }
        }
        Ok(())
    }

    fn unpin_entry(
        &self,
        repo_id: &str,
        kind: converge_model::ManifestEntryKind,
        manifests: &mut Vec<ObjectId>,
    ) -> Result<()> {
        use converge_model::{ManifestEntryKind as K, SuperpositionVariantKind as V};
        let blob = crate::storage::ObjectKind::Blob;
        match kind {
            K::File { blob: b, .. } => self.meta.unpin_object(repo_id, blob, &b)?,
            K::FileChunks { recipe: r, .. } => self.unpin_recipe(repo_id, &r)?,
            K::Dir { manifest } => manifests.push(manifest),
            K::Symlink { .. } => {}
            K::Superposition { variants } => {
                for variant in variants {
                    match variant.kind {
                        V::File { blob: b, .. } => self.meta.unpin_object(repo_id, blob, &b)?,
                        V::FileChunks { recipe: r, .. } => self.unpin_recipe(repo_id, &r)?,
                        V::Dir { manifest } => manifests.push(manifest),
                        V::Symlink { .. } | V::Tombstone => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn unpin_recipe(&self, repo_id: &str, id: &ObjectId) -> Result<()> {
        self.meta
            .unpin_object(repo_id, crate::storage::ObjectKind::Recipe, id)?;
        let Ok(bytes) = self.objects.get(crate::storage::ObjectKind::Recipe, id) else {
            return Ok(());
        };
        let recipe: converge_model::FileRecipe = converge_model::encoding::decode_recipe(&bytes)?;
        for chunk in recipe.chunks {
            self.meta
                .unpin_object(repo_id, crate::storage::ObjectKind::Blob, &chunk.blob)?;
        }
        Ok(())
    }

    /// Deterministic candidate build over the given window (doc 17 §3): fold
    /// the window's publications onto W. Pure compute against the object
    /// store — metadata writes happen in the caller's atomic batch.
    fn build_candidate(
        &self,
        authz: &AuthzContext,
        gate_id: &str,
        partition: &PartitionState,
        window: &[(u64, PublicationRecord)],
    ) -> Result<StoredCandidate> {
        assert!(!window.is_empty(), "publish composes at least its own");

        let graph = self.meta.get_gate_graph(authz.repo_id())?;
        let strategy = graph
            .gates
            .iter()
            .find(|g| g.gate_id == gate_id)
            .map(|g| g.strategy.clone())
            .unwrap_or_else(|| "whole-file".to_string());

        let w_root = match &partition.base_candidate_id {
            Some(id) => self.meta.get_candidate(id)?.root_manifest,
            None => None,
        };

        let inputs: Result<Vec<MergeInput>> = window
            .iter()
            .map(|(_, p)| {
                let base = match &p.base_candidate_id {
                    Some(id) => self.meta.get_candidate(id)?.root_manifest,
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
            candidate_hash(gate_id, w_root.as_ref(), &input_ids, &strategy, root)
        };

        let candidate = match inputs.and_then(|inputs| {
            crate::merge::merge_window_outcome(self.objects, w_root.as_ref(), &inputs, &strategy)
        }) {
            // The fold reports its own superpositions (batch 15.1, audit
            // 2.2) — no second walk over the merged tree.
            Ok(outcome) => {
                let root = outcome.root;
                let has_superpositions = outcome.has_superpositions;
                StoredCandidate {
                    candidate_id: hash_id(Some(&root)),
                    repo_id: authz.repo_id().to_string(),
                    scope_id: authz.scope_id().to_string(),
                    gate_id: gate_id.to_string(),
                    inputs: input_ids,
                    root_manifest: Some(root),
                    base_candidate_id: partition.base_candidate_id.clone(),
                    window: window_range,
                    strategy,
                    status: CandidateStatus::Ready {
                        promotable: !has_superpositions,
                    },
                    created_at: now(),
                }
            }
            Err(err) => StoredCandidate {
                candidate_id: hash_id(None),
                repo_id: authz.repo_id().to_string(),
                scope_id: authz.scope_id().to_string(),
                gate_id: gate_id.to_string(),
                inputs: input_ids,
                root_manifest: None,
                base_candidate_id: partition.base_candidate_id.clone(),
                window: window_range,
                strategy,
                status: CandidateStatus::Failed {
                    reason: format!("{err:#}"),
                },
                created_at: now(),
            },
        };
        Ok(candidate)
    }

    /// Resolve `lane_id` to a registered lane the subject may write:
    /// `None` auto-provisions the personal lane; named lanes require
    /// owner/membership (shared with publish and lane-head pushes).
    pub(crate) fn resolve_writable_lane(
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
}
