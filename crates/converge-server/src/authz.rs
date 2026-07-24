use anyhow::{Result, bail};

use crate::storage::MetadataStore;

/// Capabilities a grant can carry (arch 14 §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    Read,
    /// Sync unpublished work (snap records, lane heads) without the right
    /// to publish into a gate (arch 14 §4).
    SnapSync,
    Publish,
    Resolve,
    Approve,
    Promote,
    Release,
    Admin,
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Capability::Read => "read",
            Capability::SnapSync => "snap-sync",
            Capability::Publish => "publish",
            Capability::Resolve => "resolve",
            Capability::Approve => "approve",
            Capability::Promote => "promote",
            Capability::Release => "release",
            Capability::Admin => "admin",
        }
    }
}

/// Proof that one subject holds one capability on one (repo, scope).
///
/// The only constructor is [`authorize`], and every data-plane engine method
/// takes an `AuthzContext` by value — an operation without an authz decision
/// does not typecheck (arch 14: "no endpoint ships before its grant check
/// exists", enforced structurally).
#[derive(Debug)]
pub struct AuthzContext {
    subject: String,
    repo_id: String,
    scope_id: String,
    capability: Capability,
}

impl AuthzContext {
    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn repo_id(&self) -> &str {
        &self.repo_id
    }

    pub fn scope_id(&self) -> &str {
        &self.scope_id
    }

    pub fn capability(&self) -> Capability {
        self.capability
    }
}

pub fn authorize(
    meta: &dyn MetadataStore,
    subject: &str,
    repo_id: &str,
    scope_id: &str,
    capability: Capability,
) -> Result<AuthzContext> {
    // Implication is minimal and explicit (arch 14 §4): publish subsumes
    // snap-sync; admin subsumes everything; nothing else implies.
    let mut satisfying = vec![capability, Capability::Admin];
    if capability == Capability::SnapSync {
        satisfying.push(Capability::Publish);
    }
    let mut granted = false;
    for candidate in satisfying {
        if meta.has_grant(subject, repo_id, scope_id, candidate.as_str())? {
            granted = true;
            break;
        }
    }
    if !granted {
        bail!(
            "authorization denied: {subject} lacks {} on {repo_id}/{scope_id}",
            capability.as_str()
        );
    }
    // Scopes are declared repo state (batch 14.3, audit 2.4/M3): an
    // unregistered scope id would otherwise mint a fresh partition and
    // silently fragment windows. `*` is the repo-wide sentinel used by
    // operations that name no single scope, so it is not a scope lookup.
    if scope_id != "*" && !meta.scope_exists(repo_id, scope_id)? {
        let known = meta.list_scopes(repo_id)?;
        bail!(
            "unknown scope {scope_id} in repo {repo_id}; registered scopes: {}",
            if known.is_empty() {
                "(none)".to_string()
            } else {
                known.join(", ")
            }
        );
    }
    Ok(AuthzContext {
        subject: subject.to_string(),
        repo_id: repo_id.to_string(),
        scope_id: scope_id.to_string(),
        capability,
    })
}
