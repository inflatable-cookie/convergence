//! The inbox report.

use anyhow::Result;

use converge_model::{CandidateStatus, InboxCandidate, InboxLane, InboxPublication, InboxReport};

use crate::authz::{AuthzContext, Capability};

use super::require;

use super::{Engine, INBOX_CONTRIBUTOR_SCAN};

impl Engine<'_> {
    /// Triage report: readable lane heads (newer than `since`), the
    /// scope's current-window publications, and candidates awaiting action.
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

        // At most one candidate per gate, straight from the store: the old
        // full-scope scan read every candidate ever built here to answer a
        // question about a handful of gates (audit 4.4 / L6).
        let latest: std::collections::BTreeMap<String, crate::storage::StoredCandidate> = self
            .meta
            .latest_candidates_per_gate(authz.repo_id(), authz.scope_id())?
            .into_iter()
            .map(|candidate| (candidate.gate_id.clone(), candidate))
            .collect();
        for (gate_id, candidate) in latest {
            let approvals = self.meta.count_approvals(&candidate.candidate_id)?;

            // Where this candidate has already got to, so a gate it has
            // reached is not offered again (26.4 semantics).
            let mut reached = vec![candidate.gate_id.clone()];
            reached.extend(
                self.meta
                    .list_promotions(&candidate.candidate_id)?
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
            // of*, not the one that produced the candidate. Reading it off
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

            let recommendation = match candidate.status {
                CandidateStatus::Ready { promotable: false } => "resolve",
                CandidateStatus::Ready { promotable: true } if approvals < required => "approve",
                // Ready, approved, and a stage ahead of it. Under a
                // single gate this state was correctly silent — there was
                // nowhere to promote to — so the inbox never learned to
                // report it, and batch 26.5 found a staged repo where the
                // one thing waiting on a person was the one thing the
                // action queue did not mention.
                CandidateStatus::Ready { promotable: true } if has_somewhere_to_go => "promote",
                _ => continue,
            };
            // Who is waiting on this candidate: whoever published into it.
            // Bounded, because a wide window would turn one inbox call
            // into a hundred record reads to produce a label nobody
            // reads past the second name.
            let mut contributors: Vec<String> = Vec::new();
            // The newest input names the candidate: a candidate is a derived
            // artifact, so its human title is the last thing that went
            // into it — the snap message where one was written, the
            // publish note otherwise. Inputs are window-ordered, so the
            // newest is the last, and the walk is already bounded.
            let mut title = String::new();
            for publication_id in candidate.inputs.iter().rev().take(INBOX_CONTRIBUTOR_SCAN) {
                let Some(publication) = self.meta.get_publication(publication_id)? else {
                    continue;
                };
                if title.is_empty() {
                    if let Some(snap) = self
                        .meta
                        .get_snap_record(authz.repo_id(), &publication.snap_id)?
                        && let Some(message) = snap.message.filter(|m| !m.is_empty())
                    {
                        title = message;
                    } else if let Some(note) = publication.notes.clone().filter(|n| !n.is_empty()) {
                        title = note;
                    }
                }
                if !contributors.contains(&publication.publisher) {
                    contributors.push(publication.publisher);
                }
            }
            if title.is_empty() {
                title = format!(
                    "{} publication(s) into {gate_id}",
                    candidate.window.1.saturating_sub(candidate.window.0) + 1
                );
            }
            report.candidates.push(InboxCandidate {
                candidate_id: candidate.candidate_id,
                title,
                window: candidate.window,
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
}
