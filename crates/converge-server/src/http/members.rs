//! Repo membership and capabilities.

use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;

use crate::authz::Capability;

use crate::http::secrets::summarize_secret;

use super::{
    ApiError, DEFAULT_TOKEN_DAYS, SharedState, authorize_repo, bad_request, internal_error,
    mint_token, now_rfc3339, token_hash,
};

/// Add a teammate: upsert, grant, and optionally issue a token (batch
/// 16.3, audit P1.6). Repo admins only.
pub(crate) async fn add_member(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<converge_model::AddMemberRequest>,
) -> Result<Json<converge_model::MemberAdded>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Admin)?;
    let subject = authz.subject().to_string();
    if request.subject.is_empty() {
        return Err(bad_request("member subject is required"));
    }
    if request.capabilities.is_empty() {
        return Err(bad_request("at least one capability is required"));
    }
    // Unknown capability strings would sit in the table forever granting
    // nothing, so they are refused rather than stored. The list comes
    // from the enum: a hand-written copy is how `secret` ended up
    // ungrantable for two roadmaps.
    for capability in &request.capabilities {
        if !Capability::ALL.iter().any(|c| c.as_str() == capability) {
            return Err(bad_request(format!(
                "unknown capability {capability}; known: {}",
                Capability::ALL
                    .iter()
                    .map(Capability::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }
    let scope_pattern = if request.scope_pattern.is_empty() {
        "*".to_string()
    } else {
        request.scope_pattern.clone()
    };

    state
        .meta
        .upsert_user(&request.subject)
        .map_err(internal_error)?;
    for capability in &request.capabilities {
        state
            .meta
            .add_grant(&request.subject, &repo, &scope_pattern, capability)
            .map_err(internal_error)?;
    }

    let mut expires_at = String::new();
    let token = if request.issue_token {
        let token = mint_token()?;
        // A finite lifetime by default (batch 21.1): a credential that
        // never expires is only ever revoked by someone noticing.
        let days = request.expires_in_days.unwrap_or(DEFAULT_TOKEN_DAYS);
        let now = time::OffsetDateTime::now_utc();
        expires_at = if days == 0 {
            String::new()
        } else {
            (now + time::Duration::days(days as i64))
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|err| internal_error(anyhow::anyhow!("format expiry: {err}")))?
        };
        let hash = token_hash(&token);
        let record = converge_model::TokenRecord {
            token_id: hash.chars().take(12).collect(),
            subject: request.subject.clone(),
            label: format!("issued by {subject}"),
            issued_at: now_rfc3339()?,
            issued_by: subject.clone(),
            repo_id: repo.clone(),
            expires_at: expires_at.clone(),
            last_used_at: String::new(),
            revoked_at: String::new(),
            revoked_by: String::new(),
            revoked_reason: String::new(),
            capabilities: Vec::new(),
        };
        state
            .meta
            .create_token_record(&hash, &record)
            .map_err(internal_error)?;
        let _ = state.meta.add_event(
            &repo,
            "token.issued",
            &format!("{}/{}", record.subject, record.token_id),
            &record.issued_at,
        );
        Some(token)
    } else {
        None
    };

    Ok(Json(converge_model::MemberAdded {
        subject: request.subject,
        granted: request.capabilities,
        token,
        token_expires_at: expires_at,
    }))
}

/// Remove every grant a subject holds in this repo (batch 20.2).
///
/// What this does *not* do is re-encrypt anything: the server holds no
/// key that opens a secret (doc 19 §7), so the response names the
/// secrets still sealed to them and leaves the work with the owners who
/// can actually do it. Reporting it is the honest alternative to
/// pretending access was withdrawn.
pub(crate) async fn remove_member(
    State(state): State<SharedState>,
    Path((repo, subject)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<converge_model::MemberRemoved>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Admin)?;
    if authz.subject() == subject {
        return Err(bad_request(
            "removing yourself would leave the repo without the admin doing it",
        ));
    }

    let grants = state.meta.list_grants(&repo).map_err(internal_error)?;
    let admins: Vec<&String> = grants
        .iter()
        .filter(|(_, capability, _)| capability == Capability::Admin.as_str())
        .map(|(subject, _, _)| subject)
        .collect();
    if admins.len() == 1 && admins[0] == &subject {
        return Err(bad_request(
            "that is the repo's only admin; grant admin to someone else first",
        ));
    }

    let removed = state
        .meta
        .remove_grants(&repo, &subject)
        .map_err(internal_error)?;

    // Which secrets are still sealed to them, so somebody can act.
    let keys = state.meta.list_public_keys(&repo).map_err(internal_error)?;
    let their_keys: Vec<&str> = keys
        .iter()
        .filter(|key| key.subject == subject)
        .map(|key| key.key_id.as_str())
        .collect();
    let still_sealed: Vec<converge_model::SecretSummary> = state
        .meta
        .list_secrets(&repo)
        .map_err(internal_error)?
        .iter()
        .filter(|record| {
            record.owner == subject
                || record
                    .recipients
                    .iter()
                    .any(|key_id| their_keys.contains(&key_id.as_str()))
        })
        .map(summarize_secret)
        .collect();

    Ok(Json(converge_model::MemberRemoved {
        subject,
        grants_removed: removed,
        still_sealed,
    }))
}

pub(crate) async fn list_members(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<converge_model::MemberRecord>>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    let mut members: Vec<converge_model::MemberRecord> = Vec::new();
    for (subject, capability, scope_pattern) in
        state.meta.list_grants(&repo).map_err(internal_error)?
    {
        match members.last_mut() {
            Some(member) if member.subject == subject => {
                member.grants.push((capability, scope_pattern))
            }
            _ => members.push(converge_model::MemberRecord {
                subject,
                grants: vec![(capability, scope_pattern)],
            }),
        }
    }
    Ok(Json(members))
}
