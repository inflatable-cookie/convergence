use anyhow::{Result, bail};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use converge_model::{
    BundleStatus, InboxBundle, InboxLane, InboxPublication, InboxReport, LaneHead, LaneRecord,
    ObjectId, PublicationRecord, ReleaseRecord, SnapRecord, VerifyReport,
};

use crate::authz::{AuthzContext, Capability};

/// How many of a bundle's inputs the inbox reads to name contributors
/// (batch 23.4).
///
/// The label is "who is waiting on this", and nobody reads past the
/// second name — but a coalesced window can hold a hundred
/// publications, and reading all of them per gate per inbox call would
/// make a cosmetic label the most expensive thing in the response.
/// Capped rather than uncapped-and-regretted, and the cap is stated on
/// the wire type so a client knows the list is partial.
const INBOX_CONTRIBUTOR_SCAN: usize = 8;
use crate::merge::{MergeInput, merge_window};
use crate::storage::{
    BatchConflict, MetaOp, MetadataStore, ObjectStore, PartitionState, StoredBundle,
};

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

        // One atomic operation per attempt (batch 13.1, audit H2): read the
        // partition, compute the publication + merged bundle in memory, then
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
                base_bundle_id: input.base_bundle_id.clone(),
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
            let bundle = self.build_bundle(&authz, &input.gate_id, &partition, &window)?;

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
                MetaOp::PutBundle(bundle.clone()),
                // Event hint (doc 14 §5b): bundle state changed.
                MetaOp::AddEvent {
                    repo_id: authz.repo_id().to_string(),
                    kind: "bundle".to_string(),
                    subject_id: bundle.bundle_id.clone(),
                    created_at: now(),
                },
            ];
            match self.meta.apply_batch(&ops) {
                Ok(()) => {
                    // The publication now references the uploaded tree
                    // durably: release its upload pins (batch 12.2).
                    self.unpin_tree(authz.repo_id(), &input.snap.root_manifest)?;
                    return Ok(bundle);
                }
                Err(err) if err.is::<BatchConflict>() => continue,
                Err(err) => return Err(err),
            }
        }
        bail!("publish kept losing to concurrent publishes after {ATTEMPTS} attempts")
    }

    /// Release upload pins for every object reachable from `root` (batch
    /// 12.2). Tolerates missing objects — a partial tree just unpins less.
    fn unpin_tree(&self, repo_id: &str, root: &ObjectId) -> Result<()> {
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

    /// Deterministic bundle build over the given window (doc 17 §3): fold
    /// the window's publications onto W. Pure compute against the object
    /// store — metadata writes happen in the caller's atomic batch.
    fn build_bundle(
        &self,
        authz: &AuthzContext,
        gate_id: &str,
        partition: &PartitionState,
        window: &[(u64, PublicationRecord)],
    ) -> Result<StoredBundle> {
        assert!(!window.is_empty(), "publish composes at least its own");

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

        let bundle = match inputs.and_then(|inputs| {
            crate::merge::merge_window_outcome(self.objects, w_root.as_ref(), &inputs, &strategy)
        }) {
            // The fold reports its own superpositions (batch 15.1, audit
            // 2.2) — no second walk over the merged tree.
            Ok(outcome) => {
                let root = outcome.root;
                let has_superpositions = outcome.has_superpositions;
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
        Ok(bundle)
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
        require(&authz, Capability::SnapSync)?;
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
        // The head lineage's trees are now referenced by a lane head:
        // release their upload pins (batch 12.2).
        let mut stack = vec![head.snap_id.clone()];
        let mut walked = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if !walked.insert(id.clone()) {
                continue;
            }
            if let Some(record) = self.meta.get_snap_record(authz.repo_id(), &id)? {
                self.unpin_tree(authz.repo_id(), &record.root_manifest)?;
                stack.extend(record.parents);
            }
        }
        self.meta
            .add_event(authz.repo_id(), "lane", &head.lane_id, &now())?;
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
        // The snap's tree must be present (batch 12.2, audit M4): otherwise
        // a lane head fast-forwarded to it would dangle and never
        // materialize. Thinned *ancestors* may be absent, but the head's own
        // root manifest may not.
        if !self
            .objects
            .has(crate::storage::ObjectKind::Manifest, &snap.root_manifest)
        {
            bail!(
                "snap {} root manifest {} not uploaded",
                snap.id,
                snap.root_manifest.as_str()
            );
        }
        self.meta.put_snap_record(authz.repo_id(), snap)
    }

    /// Triage report: readable lane heads (newer than `since`), the
    /// scope's current-window publications, and bundles awaiting action.
    pub fn inbox(&self, authz: &AuthzContext, since: Option<&str>) -> Result<InboxReport> {
        require(authz, Capability::Read)?;
        let mut report = InboxReport::default();

        // Each section is capped so a large repo cannot produce an
        // unbounded report (batch 15.2); `truncated` says when a cut
        // happened rather than passing a partial list off as complete.
        const SECTION_CAP: usize = 200;

        let mut lane_cursor: Option<String> = None;
        'lanes: loop {
            let page =
                self.meta
                    .list_lanes_page(authz.repo_id(), lane_cursor.as_deref(), SECTION_CAP)?;
            if page.is_empty() {
                break;
            }
            lane_cursor = page.last().map(|l| l.lane_id.clone());
            for lane in page {
                if self.check_lane_readable(authz, &lane.lane_id).is_err() {
                    continue;
                }
                if let Some(head) = self.meta.get_lane_head(authz.repo_id(), &lane.lane_id)? {
                    if since.is_some_and(|s| head.updated_at.as_str() <= s) {
                        continue;
                    }
                    if report.lanes.len() >= SECTION_CAP {
                        report.truncated = true;
                        break 'lanes;
                    }
                    report.lanes.push(InboxLane {
                        lane_id: lane.lane_id,
                        head_snap_id: head.snap_id,
                        updated_at: head.updated_at,
                    });
                }
            }
        }

        let graph = self.meta.get_gate_graph(authz.repo_id())?;
        'publications: for gate in &graph.gates {
            let partition =
                self.meta
                    .get_partition_state(authz.repo_id(), authz.scope_id(), &gate.gate_id)?;
            for (_, publication) in self.meta.list_publications_after(
                authz.repo_id(),
                authz.scope_id(),
                &gate.gate_id,
                partition.window_floor,
            )? {
                if report.publications.len() >= SECTION_CAP {
                    report.truncated = true;
                    break 'publications;
                }
                report.publications.push(InboxPublication {
                    gate_id: gate.gate_id.clone(),
                    publication_id: publication.publication_id,
                    lane_id: publication.lane_id,
                    publisher: publication.publisher,
                    created_at: publication.created_at,
                });
            }
        }

        // At most one bundle per gate, straight from the store: the old
        // full-scope scan read every bundle ever built here to answer a
        // question about a handful of gates (audit 4.4 / L6).
        let latest: std::collections::BTreeMap<String, crate::storage::StoredBundle> = self
            .meta
            .latest_bundles_per_gate(authz.repo_id(), authz.scope_id())?
            .into_iter()
            .map(|bundle| (bundle.gate_id.clone(), bundle))
            .collect();
        for (gate_id, bundle) in latest {
            let approvals = self.meta.count_approvals(&bundle.bundle_id)?;

            // Where this bundle has already got to, so a gate it has
            // reached is not offered again (26.4 semantics).
            let mut reached = vec![bundle.gate_id.clone()];
            reached.extend(
                self.meta
                    .list_promotions(&bundle.bundle_id)?
                    .into_iter()
                    .map(|(_, to, _)| to),
            );
            // Onward gate, paired with the gate it would be promoted out
            // of — which is the gate whose approval policy applies.
            let onward: Vec<(String, String)> = graph
                .gates
                .iter()
                .filter_map(|candidate| {
                    if reached.contains(&candidate.gate_id) {
                        return None;
                    }
                    let from = candidate.upstreams.iter().find(|up| reached.contains(up))?;
                    Some((candidate.gate_id.clone(), from.clone()))
                })
                .collect();
            let has_somewhere_to_go = !onward.is_empty();

            // Approvals are required by the gate being promoted *out
            // of*, not the one that produced the bundle. Reading it off
            // the producing gate is the same mistake batch 26.4 fixed in
            // `promote` itself, and it survived here one batch longer:
            // the inbox recommended a promotion out of a review stage as
            // `(0/0)` and the server then refused it for want of the
            // approval the inbox had not asked for.
            let from_gate = onward
                .first()
                .map(|(_, from)| from.clone())
                .unwrap_or_else(|| gate_id.clone());
            let required = graph
                .gates
                .iter()
                .find(|g| g.gate_id == from_gate)
                .map(|g| g.required_approvals)
                .unwrap_or(0);

            let recommendation = match bundle.status {
                BundleStatus::Ready { promotable: false } => "resolve",
                BundleStatus::Ready { promotable: true } if approvals < required => "approve",
                // Ready, approved, and a stage ahead of it. Under a
                // single gate this state was correctly silent — there was
                // nowhere to promote to — so the inbox never learned to
                // report it, and batch 26.5 found a staged repo where the
                // one thing waiting on a person was the one thing the
                // action queue did not mention.
                BundleStatus::Ready { promotable: true } if has_somewhere_to_go => "promote",
                _ => continue,
            };
            // Who is waiting on this bundle: whoever published into it.
            // Bounded, because a wide window would turn one inbox call
            // into a hundred record reads to produce a label nobody
            // reads past the second name.
            let mut contributors: Vec<String> = Vec::new();
            for publication_id in bundle.inputs.iter().take(INBOX_CONTRIBUTOR_SCAN) {
                if let Some(publication) = self.meta.get_publication(publication_id)?
                    && !contributors.contains(&publication.publisher)
                {
                    contributors.push(publication.publisher);
                }
            }
            report.bundles.push(InboxBundle {
                bundle_id: bundle.bundle_id,
                gate_id,
                recommendation: recommendation.to_string(),
                // Only when there is one answer. Offering a guess where
                // a person has to choose is worse than offering nothing.
                from_gate: onward.first().map(|(_, from)| from.clone()),
                next_gate: match onward.as_slice() {
                    [(only, _)] => Some(only.clone()),
                    _ => None,
                },
                approvals,
                required_approvals: required,
                contributors,
            });
        }
        Ok(report)
    }

    pub fn approve(&self, authz: AuthzContext, bundle_id: &str) -> Result<u32> {
        require(&authz, Capability::Approve)?;
        let bundle = self.meta.get_bundle(bundle_id)?;
        ensure_partition(&authz, &bundle)?;
        // The caller may have given a prefix (batch 22.4), and
        // `get_bundle` resolved it — so from here the *resolved* id is
        // the only one to use. Batch 26.4 found the alternative: promote
        // compared the partition's stored base against the short string
        // the user typed, decided a bundle was not the current window,
        // and wrote a truncated id into the promotions table that
        // referenced no real bundle.
        let bundle_id = bundle.bundle_id.as_str();
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
        // Resolved id from here (batch 26.4): see `promote`.
        let bundle_id = bundle.bundle_id.as_str();
        let graph = self.meta.get_gate_graph(authz.repo_id())?;

        // Releasable where it has *reached*, not where it was built. The
        // same assumption that made a staged graph untraversable also
        // meant a bundle promoted into a release gate could not be
        // released from it, because the check read `may_release` off the
        // gate that produced it — which in a staged graph is the entry
        // gate, and an entry gate that may release is not a staged graph.
        let mut reached = vec![bundle.gate_id.clone()];
        reached.extend(
            self.meta
                .list_promotions(bundle_id)?
                .into_iter()
                .map(|(_, to, _)| to),
        );
        if !graph
            .gates
            .iter()
            .any(|g| g.may_release && reached.contains(&g.gate_id))
        {
            bail!(
                "no gate this bundle has reached may release: {}",
                reached.join(", ")
            );
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
        self.meta
            .add_event(authz.repo_id(), "release", channel, &now())?;
        Ok(release)
    }

    /// Policy-checked promotion (arch 14 §3): target gate must list the
    /// producing gate upstream; the producing gate's required approvals must
    /// be met; the bundle must be ready and promotable.
    pub fn promote(&self, authz: AuthzContext, bundle_id: &str, to_gate: &str) -> Result<()> {
        require(&authz, Capability::Promote)?;
        let bundle = self.meta.get_bundle(bundle_id)?;
        ensure_partition(&authz, &bundle)?;
        // The caller may have given a prefix (batch 22.4), and
        // `get_bundle` resolved it — so from here the *resolved* id is
        // the only one to use. Batch 26.4 found the alternative: promote
        // compared the partition's stored base against the short string
        // the user typed, decided a bundle was not the current window,
        // and wrote a truncated id into the promotions table that
        // referenced no real bundle.
        let bundle_id = bundle.bundle_id.as_str();

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
        // Where the bundle has *got to*, not merely where it was built.
        //
        // A bundle keeps the gate that produced it for ever, and doc 14
        // §3 has always said re-promoting it "to a further downstream
        // gate records the promotion" — but this check only ever looked
        // at the producing gate, so a chain was impossible. Batch 26.4
        // drove intake -> review -> release, the first staged graph that
        // has ever existed, and the second hop was refused with "gate
        // release does not accept promotions from intake". Any gate
        // whose upstream was not an entry gate was unreachable.
        //
        // The reached set is the producing gate plus every gate a
        // recorded promotion delivered it to. Fan-out to siblings still
        // works, and skipping a stage is still refused: promoting
        // straight from intake to release fails until the bundle has
        // actually reached review.
        let mut reached = vec![bundle.gate_id.clone()];
        reached.extend(
            self.meta
                .list_promotions(bundle_id)?
                .into_iter()
                .map(|(_, to, _)| to),
        );
        let Some(from_gate) = reached
            .iter()
            .find(|gate| target.upstreams.contains(gate))
            .cloned()
        else {
            bail!(
                "gate {to_gate} does not accept promotions from {}",
                reached.join(", ")
            );
        };
        // The approval policy that applies is the one on the gate being
        // promoted *out of*. Reading it off the producing gate meant a
        // review stage's `required_approvals` was never enforced on the
        // hop that leaves it — the setting existed and did nothing.
        let producing = graph
            .gates
            .iter()
            .find(|g| g.gate_id == from_gate)
            .ok_or_else(|| anyhow::anyhow!("unknown gate {from_gate}"))?;
        let approvals = self.meta.count_approvals(bundle_id)?;
        if approvals < producing.required_approvals {
            bail!(
                "bundle {bundle_id} has {approvals} of {} required approvals",
                producing.required_approvals
            );
        }

        // One atomic operation (batch 13.1, audit H2): the promotion record
        // and the window advance commit together, guarded against the
        // partition moving under us — conflict is a clear error, not silent
        // last-writer-wins.
        let partition =
            self.meta
                .get_partition_state(authz.repo_id(), authz.scope_id(), &bundle.gate_id)?;

        // Monotonicity guards (batch 13.2, audit H1, doc 14 §3): promote
        // only advances the window. A bundle that already is the current W
        // re-promotes to another downstream gate without touching state
        // (fan-out); anything stale is refused instead of rewinding the
        // floor and re-opening consumed publications.
        let is_current_w = partition.base_bundle_id.as_deref() == Some(bundle_id)
            && partition.window_floor == bundle.window.1;
        if !is_current_w {
            if bundle.window.1 <= partition.window_floor {
                bail!(
                    "stale bundle {bundle_id}: its window ends at seq {} but the \
                     partition floor is already {} — a newer bundle was promoted; \
                     republish against the current W and promote that",
                    bundle.window.1,
                    partition.window_floor
                );
            }
            if bundle.base_bundle_id != partition.base_bundle_id {
                bail!(
                    "bundle {bundle_id} was built on base {:?} but the partition's \
                     current W is {:?} — promote would fork promoted history; \
                     republish against the current W",
                    bundle.base_bundle_id,
                    partition.base_bundle_id
                );
            }
        }

        // Idempotent retry (batch 18.1): a client whose promote timed out
        // and retried must not record the promotion twice. The state half
        // is already idempotent — `is_current_w` skips the advance — so
        // only the record needed the check. Fan-out to a *different* gate
        // still goes through.
        if self
            .meta
            .list_promotions(bundle_id)?
            .iter()
            .any(|(_, to, _)| to == to_gate)
        {
            return Ok(());
        }

        let mut ops = vec![
            MetaOp::AssertPartitionState {
                repo_id: authz.repo_id().to_string(),
                scope_id: authz.scope_id().to_string(),
                gate_id: bundle.gate_id.clone(),
                expected: partition,
            },
            MetaOp::RecordPromotion {
                bundle_id: bundle_id.to_string(),
                from_gate: bundle.gate_id.clone(),
                to_gate: to_gate.to_string(),
                at: now(),
            },
        ];
        if !is_current_w {
            // Promotion advances the window (doc 17 §3): the promoted bundle
            // becomes W and its window's publications leave the pool.
            ops.push(MetaOp::SetPartitionState {
                repo_id: authz.repo_id().to_string(),
                scope_id: authz.scope_id().to_string(),
                gate_id: bundle.gate_id.clone(),
                state: PartitionState {
                    window_floor: bundle.window.1,
                    base_bundle_id: Some(bundle.bundle_id.clone()),
                },
            });
        }
        self.meta.apply_batch(&ops).map_err(|err| {
            if err.is::<BatchConflict>() {
                anyhow::anyhow!(
                    "partition advanced concurrently; re-check and retry promote: {err}"
                )
            } else {
                err
            }
        })
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
