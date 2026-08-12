use anyhow::{Result, bail};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use converge_model::{ObjectId, SnapRecord};

use crate::authz::{AuthzContext, Capability};

/// How many of a candidate's inputs the inbox reads to name contributors
/// (batch 23.4).
///
/// The label is "who is waiting on this", and nobody reads past the
/// second name — but a coalesced window can hold a hundred
/// publications, and reading all of them per gate per inbox call would
/// make a cosmetic label the most expensive thing in the response.
/// Capped rather than uncapped-and-regretted, and the cap is stated on
/// the wire type so a client knows the list is partial.
const INBOX_CONTRIBUTOR_SCAN: usize = 8;
use crate::storage::{MetadataStore, ObjectStore, StoredCandidate};

/// The convergence engine: publish intake, deterministic candidate builds, and
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
    /// The candidate the publisher last saw for this target (doc 17 §2).
    pub base_candidate_id: Option<String>,
    /// `None` -> the publisher's auto-provisioned personal lane.
    pub lane_id: Option<String>,
    pub notes: Option<String>,
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("format rfc3339")
}

impl Engine<'_> {}

mod flow;
mod gates;
mod inbox;
mod publish;

/// Deterministic candidate identity (doc 17 §3): hash(gate, W root, ordered
/// input publication ids, strategy, merged root).
pub fn candidate_hash(
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

fn ensure_partition(authz: &AuthzContext, candidate: &StoredCandidate) -> Result<()> {
    if candidate.repo_id != authz.repo_id() || candidate.scope_id != authz.scope_id() {
        bail!(
            "candidate {} belongs to {}/{}, authz covers {}/{}",
            candidate.candidate_id,
            candidate.repo_id,
            candidate.scope_id,
            authz.repo_id(),
            authz.scope_id()
        );
    }
    Ok(())
}
