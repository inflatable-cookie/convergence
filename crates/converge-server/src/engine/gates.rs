//! The promotion DAG: approve, promote, release, verify, yank.

use anyhow::{Result, bail};

use converge_model::{CandidateStatus, ReleaseRecord, VerifyReport};

use crate::authz::{AuthzContext, Capability};

use super::{candidate_hash, ensure_partition, now, require};

use crate::merge::{MergeInput, merge_window};

use crate::storage::{BatchConflict, MetaOp, PartitionState};

use super::Engine;

impl Engine<'_> {
    pub fn approve(&self, authz: AuthzContext, candidate_id: &str) -> Result<u32> {
        require(&authz, Capability::Approve)?;
        let candidate = self.meta.get_candidate(candidate_id)?;
        ensure_partition(&authz, &candidate)?;
        // The caller may have given a prefix (batch 22.4), and
        // `get_candidate` resolved it — so from here the *resolved* id is
        // the only one to use. Batch 26.4 found the alternative: promote
        // compared the partition's stored base against the short string
        // the user typed, decided a candidate was not the current window,
        // and wrote a truncated id into the promotions table that
        // referenced no real candidate.
        let candidate_id = candidate.candidate_id.as_str();
        self.meta.add_approval(candidate_id, authz.subject())?;
        self.meta.count_approvals(candidate_id)
    }

    /// Provenance replay (vision: determinism as a product feature):
    /// re-run the recorded merge and prove the candidate's identity.
    pub fn verify(&self, candidate_id: &str) -> Result<VerifyReport> {
        let candidate = self.meta.get_candidate(candidate_id)?;
        let w_root = match &candidate.base_candidate_id {
            Some(id) => self.meta.get_candidate(id)?.root_manifest,
            None => None,
        };
        let mut inputs = Vec::new();
        for publication_id in &candidate.inputs {
            let publication = self.meta.get_publication(publication_id)?.ok_or_else(|| {
                anyhow::anyhow!("provenance incomplete: publication {publication_id} missing")
            })?;
            let base = match &publication.base_candidate_id {
                Some(id) => self.meta.get_candidate(id)?.root_manifest,
                None => None,
            };
            inputs.push(MergeInput {
                lane: publication.lane_id,
                base,
                tree: publication.root_manifest,
            });
        }
        let recomputed_root =
            merge_window(self.objects, w_root.as_ref(), &inputs, &candidate.strategy)?;
        let recomputed_id = candidate_hash(
            &candidate.gate_id,
            w_root.as_ref(),
            &candidate.inputs,
            &candidate.strategy,
            Some(&recomputed_root),
        );
        let root_matches = candidate.root_manifest.as_ref() == Some(&recomputed_root);
        let id_matches = recomputed_id == candidate.candidate_id;
        Ok(VerifyReport {
            verified: root_matches && id_matches,
            candidate_id: candidate.candidate_id.clone(),
            recorded_root: candidate.root_manifest.clone(),
            recomputed_root: Some(recomputed_root),
            recomputed_id,
            detail: if root_matches && id_matches {
                "replayed merge reproduces the recorded candidate".to_string()
            } else if !root_matches {
                "recomputed root manifest differs from the recorded one".to_string()
            } else {
                "recomputed candidate id differs from the recorded one".to_string()
            },
        })
    }

    /// The sixth verb: designate a ready, promotable candidate for
    /// consumption on a named channel. Policy: the producing gate must be
    /// marked `may_release` (vision: release is a policy-driven output,
    /// not the terminal gate by definition).
    pub fn release(
        &self,
        authz: AuthzContext,
        candidate_id: &str,
        channel: &str,
        notes: Option<String>,
    ) -> Result<ReleaseRecord> {
        require(&authz, Capability::Release)?;
        let candidate = self.meta.get_candidate(candidate_id)?;
        ensure_partition(&authz, &candidate)?;
        match &candidate.status {
            CandidateStatus::Ready { promotable: true } => {}
            CandidateStatus::Ready { promotable: false } => {
                bail!("candidate {candidate_id} has unresolved superpositions")
            }
            other => bail!("candidate {candidate_id} is not ready: {other:?}"),
        }
        // Resolved id from here (batch 26.4): see `promote`.
        let candidate_id = candidate.candidate_id.as_str();
        let graph = self.meta.get_gate_graph(authz.repo_id())?;

        // Releasable where it has *reached*, not where it was built. The
        // same assumption that made a staged graph untraversable also
        // meant a candidate promoted into a release gate could not be
        // released from it, because the check read `may_release` off the
        // gate that produced it — which in a staged graph is the entry
        // gate, and an entry gate that may release is not a staged graph.
        let mut reached = vec![candidate.gate_id.clone()];
        reached.extend(
            self.meta
                .list_promotions(candidate_id)?
                .into_iter()
                .map(|(_, to, _)| to),
        );
        if !graph
            .gates
            .iter()
            .any(|g| g.may_release && reached.contains(&g.gate_id))
        {
            bail!(
                "no gate this candidate has reached may release: {}",
                reached.join(", ")
            );
        }
        // Semver identity (g02.028): valid, and unique per repo —
        // checked here so the refusal happens before anything is
        // written. Uniqueness only: backports below `latest` are how
        // long-term support works, and strictness is a later opt-in.
        let version =
            converge_model::releases::parse_version(channel).map_err(|err| anyhow::anyhow!(err))?;
        let existing: Vec<semver::Version> = self
            .meta
            .list_releases(authz.repo_id())?
            .iter()
            .filter_map(|r| converge_model::releases::parse_version(&r.version).ok())
            .collect();
        if let Some(refusal) = converge_model::releases::refuse_version(&version, &existing) {
            bail!(refusal);
        }
        let release = ReleaseRecord {
            version: version.to_string(),
            yanked: false,
            yank_reason: None,
            repo_id: authz.repo_id().to_string(),
            scope_id: authz.scope_id().to_string(),
            candidate_id: candidate_id.to_string(),
            released_by: authz.subject().to_string(),
            notes,
            created_at: now(),
        };
        self.meta.add_release(&release)?;
        self.meta
            .add_event(authz.repo_id(), "release", &release.version, &now())?;
        Ok(release)
    }

    /// Withdraw a release (g02.028): marked, never deleted. It leaves
    /// `latest` and range resolution; an exact version still reaches it.
    pub fn yank(&self, authz: AuthzContext, version: &str, reason: &str) -> Result<()> {
        require(&authz, Capability::Release)?;
        let version = converge_model::releases::parse_version(version)
            .map_err(|err| anyhow::anyhow!(err))?
            .to_string();
        if !self
            .meta
            .set_release_yanked(authz.repo_id(), &version, reason)?
        {
            bail!("no release {version}");
        }
        self.meta
            .add_event(authz.repo_id(), "release.yanked", &version, &now())?;
        Ok(())
    }

    /// Policy-checked promotion (arch 14 §3): target gate must list the
    /// producing gate upstream; the producing gate's required approvals must
    /// be met; the candidate must be ready and promotable.
    pub fn promote(&self, authz: AuthzContext, candidate_id: &str, to_gate: &str) -> Result<()> {
        require(&authz, Capability::Promote)?;
        let candidate = self.meta.get_candidate(candidate_id)?;
        ensure_partition(&authz, &candidate)?;
        // The caller may have given a prefix (batch 22.4), and
        // `get_candidate` resolved it — so from here the *resolved* id is
        // the only one to use. Batch 26.4 found the alternative: promote
        // compared the partition's stored base against the short string
        // the user typed, decided a candidate was not the current window,
        // and wrote a truncated id into the promotions table that
        // referenced no real candidate.
        let candidate_id = candidate.candidate_id.as_str();

        match &candidate.status {
            CandidateStatus::Ready { promotable: true } => {}
            CandidateStatus::Ready { promotable: false } => {
                bail!("candidate {candidate_id} has unresolved superpositions")
            }
            other => bail!("candidate {candidate_id} is not ready: {other:?}"),
        }

        let graph = self.meta.get_gate_graph(authz.repo_id())?;
        let target = graph
            .gates
            .iter()
            .find(|g| g.gate_id == to_gate)
            .ok_or_else(|| anyhow::anyhow!("unknown target gate {to_gate}"))?;
        // Where the candidate has *got to*, not merely where it was built.
        //
        // A candidate keeps the gate that produced it for ever, and doc 14
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
        // straight from intake to release fails until the candidate has
        // actually reached review.
        let mut reached = vec![candidate.gate_id.clone()];
        reached.extend(
            self.meta
                .list_promotions(candidate_id)?
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
        let approvals = self.meta.count_approvals(candidate_id)?;
        if approvals < producing.required_approvals {
            bail!(
                "candidate {candidate_id} has {approvals} of {} required approvals",
                producing.required_approvals
            );
        }

        // One atomic operation (batch 13.1, audit H2): the promotion record
        // and the window advance commit together, guarded against the
        // partition moving under us — conflict is a clear error, not silent
        // last-writer-wins.
        let partition =
            self.meta
                .get_partition_state(authz.repo_id(), authz.scope_id(), &candidate.gate_id)?;

        // Monotonicity guards (batch 13.2, audit H1, doc 14 §3): promote
        // only advances the window. A candidate that already is the current W
        // re-promotes to another downstream gate without touching state
        // (fan-out); anything stale is refused instead of rewinding the
        // floor and re-opening consumed publications.
        let is_current_w = partition.base_candidate_id.as_deref() == Some(candidate_id)
            && partition.window_floor == candidate.window.1;
        if !is_current_w {
            if candidate.window.1 <= partition.window_floor {
                bail!(
                    "stale candidate {candidate_id}: its window ends at seq {} but the \
                     partition floor is already {} — a newer candidate was promoted; \
                     republish against the current W and promote that",
                    candidate.window.1,
                    partition.window_floor
                );
            }
            if candidate.base_candidate_id != partition.base_candidate_id {
                bail!(
                    "candidate {candidate_id} was built on base {:?} but the partition's \
                     current W is {:?} — promote would fork promoted history; \
                     republish against the current W",
                    candidate.base_candidate_id,
                    partition.base_candidate_id
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
            .list_promotions(candidate_id)?
            .iter()
            .any(|(_, to, _)| to == to_gate)
        {
            return Ok(());
        }

        let mut ops = vec![
            MetaOp::AssertPartitionState {
                repo_id: authz.repo_id().to_string(),
                scope_id: authz.scope_id().to_string(),
                gate_id: candidate.gate_id.clone(),
                expected: partition,
            },
            MetaOp::RecordPromotion {
                candidate_id: candidate_id.to_string(),
                from_gate: candidate.gate_id.clone(),
                to_gate: to_gate.to_string(),
                at: now(),
            },
        ];
        if !is_current_w {
            // Promotion advances the window (doc 17 §3): the promoted candidate
            // becomes W and its window's publications leave the pool.
            ops.push(MetaOp::SetPartitionState {
                repo_id: authz.repo_id().to_string(),
                scope_id: authz.scope_id().to_string(),
                gate_id: candidate.gate_id.clone(),
                state: PartitionState {
                    window_floor: candidate.window.1,
                    base_candidate_id: Some(candidate.candidate_id.clone()),
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
