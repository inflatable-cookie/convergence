use anyhow::{Result, bail};

use crate::storage::MetadataStore;

/// Capabilities a grant can carry (arch 14 §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    Read,
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
    let granted = meta.has_grant(subject, repo_id, scope_id, capability.as_str())?
        || meta.has_grant(subject, repo_id, scope_id, Capability::Admin.as_str())?;
    if !granted {
        bail!(
            "authorization denied: {subject} lacks {} on {repo_id}/{scope_id}",
            capability.as_str()
        );
    }
    Ok(AuthzContext {
        subject: subject.to_string(),
        repo_id: repo_id.to_string(),
        scope_id: scope_id.to_string(),
        capability,
    })
}
