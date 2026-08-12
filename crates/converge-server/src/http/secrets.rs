//! Encrypted secrets: read, write, share state, recipients.

use axum::Json;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;

use crate::authz::Capability;

use super::{
    ApiError, AppState, SharedState, authorize_repo, bad_request, forbidden, internal_error,
    now_rfc3339,
};

/// Store an encrypted secret (batch 19.2).
///
/// The server is an envelope service: it validates the *shape* of the
/// request and never the ciphertext, which it stores and returns
/// byte-exact.
pub(crate) async fn set_secret(
    State(state): State<SharedState>,
    Path((repo, name)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<converge_model::SetSecretRequest>,
) -> Result<Json<converge_model::SecretSummary>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Secret)?;
    let subject = authz.subject().to_string();
    validate_secret_name(&name)?;
    if request.ciphertext.is_empty() {
        return Err(bad_request("ciphertext is empty"));
    }
    if request.recipients.is_empty() {
        return Err(bad_request(
            "a secret with no recipients could never be read again",
        ));
    }

    let now = now_rfc3339()?;
    // A re-share keeps the previous value version and timestamp, so an
    // audit can still answer "when was this credential last changed?"
    // after any number of membership edits.
    let previous = state
        .meta
        .get_secret(&repo, &subject, &name)
        .map_err(internal_error)?;
    let (value_version, value_updated_at) = match (&previous, request.value_changed) {
        (_, true) => (
            previous.as_ref().map(|p| p.value_version).unwrap_or(0) + 1,
            now.clone(),
        ),
        (Some(previous), false) => (previous.value_version, previous.value_updated_at.clone()),
        (None, false) => (1, now.clone()),
    };

    let record = converge_model::SecretRecord {
        name: name.clone(),
        owner: subject.clone(),
        recipients: request.recipients,
        ciphertext: request.ciphertext,
        version: request.expected_version + 1,
        value_version,
        value_updated_at,
        updated_at: now,
        updated_by: subject.clone(),
    };
    // Guarded like publish and promote (doc 14 §3): a stale write fails
    // the batch instead of quietly winning, so two people rotating the
    // same credential cannot lose one of the rotations.
    state
        .meta
        .apply_batch(&[
            crate::storage::MetaOp::AssertSecretVersion {
                repo_id: repo.clone(),
                owner: subject.clone(),
                name: name.clone(),
                expected: request.expected_version,
            },
            crate::storage::MetaOp::PutSecret {
                repo_id: repo.clone(),
                record: record.clone(),
            },
            crate::storage::MetaOp::AddEvent {
                repo_id: repo.clone(),
                kind: "secret.changed".into(),
                subject_id: format!("{subject}/{name}@{}", record.version),
                created_at: record.updated_at.clone(),
            },
        ])
        .map_err(|err| {
            if err.is::<crate::storage::BatchConflict>() {
                ApiError(StatusCode::CONFLICT, format!("{err}"))
            } else {
                internal_error(err)
            }
        })?;
    Ok(Json(summarize_secret(&record)))
}

/// Fetch ciphertext. Only a recipient may, which the grant check alone
/// does not give us: `admin` subsumes every capability (doc 14 §4), so
/// without this a repo admin could pull every envelope in the repo.
pub(crate) async fn get_secret(
    State(state): State<SharedState>,
    Path((repo, name)): Path<(String, String)>,
    Query(params): Query<SecretQuery>,
    headers: HeaderMap,
) -> Result<Json<converge_model::SecretRecord>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Secret)?;
    let subject = authz.subject().to_string();
    let record = find_secret(&state, &repo, &name, &subject, params.owner.as_deref())?;

    let keys = state.meta.list_public_keys(&repo).map_err(internal_error)?;
    if !is_recipient(&record, &subject, &keys) {
        // Same shape as "no such secret": whether a secret exists is
        // itself information a non-recipient has no claim to.
        return Err(not_found_secret(&name));
    }

    // Every fetch that enables a decryption is on the record (doc 19
    // §10c). A file on disk cannot tell you it was read; this can, and
    // that is what turns a leaked credential into a bounded incident.
    // The trade is deliberate: the server learns when each person uses
    // each secret, which §10c chooses over read-privacy.
    let at = now_rfc3339()?;
    state
        .meta
        .add_event(
            &repo,
            "secret.read",
            &format!("{subject}/{name}@{}", record.version),
            &at,
        )
        .map_err(internal_error)?;
    Ok(Json(record))
}

/// Names and versions, never ciphertext. Deliberately readable by any
/// member: knowing that a secret exists is what lets someone ask to be
/// added to it.
pub(crate) async fn list_secrets(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<converge_model::SecretSummary>>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    Ok(Json(
        state
            .meta
            .list_secrets(&repo)
            .map_err(internal_error)?
            .iter()
            .map(summarize_secret)
            .collect(),
    ))
}

/// Only the owner may delete: a recipient can read a secret, not
/// destroy someone else's copy of it.
pub(crate) async fn delete_secret(
    State(state): State<SharedState>,
    Path((repo, name)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Secret)?;
    let subject = authz.subject().to_string();
    let record = find_secret(&state, &repo, &name, &subject, Some(&subject))?;
    if record.owner != subject {
        return Err(forbidden(format!("{name} belongs to {}", record.owner)));
    }
    state
        .meta
        .delete_secret(&repo, &subject, &name)
        .map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "deleted": name })))
}

/// `?owner=` disambiguates when two people hold the same name.
#[derive(serde::Deserialize)]
pub(crate) struct SecretQuery {
    owner: Option<String>,
}

pub(crate) fn summarize_secret(
    record: &converge_model::SecretRecord,
) -> converge_model::SecretSummary {
    converge_model::SecretSummary {
        name: record.name.clone(),
        owner: record.owner.clone(),
        recipients: record.recipients.clone(),
        version: record.version,
        value_version: record.value_version,
        value_updated_at: record.value_updated_at.clone(),
        updated_at: record.updated_at.clone(),
        updated_by: record.updated_by.clone(),
    }
}

fn not_found_secret(name: &str) -> ApiError {
    ApiError(StatusCode::NOT_FOUND, format!("no secret {name}"))
}

/// Resolve a secret by name for a caller (batch 20.1).
///
/// Records are keyed `(repo, owner, name)`, so a name alone is not an
/// address once more than one person can hold a secret. Resolution
/// prefers the caller's own, falls back to the single one they can
/// read, and refuses rather than guessing when several match — the
/// previous code took the first by owner order, which meant two people
/// with a `db-password` silently served whoever sorted first.
fn find_secret(
    state: &AppState,
    repo: &str,
    name: &str,
    subject: &str,
    owner: Option<&str>,
) -> Result<converge_model::SecretRecord, ApiError> {
    let all = state.meta.list_secrets(repo).map_err(internal_error)?;
    let matching: Vec<converge_model::SecretRecord> = all
        .into_iter()
        .filter(|record| record.name == name)
        .filter(|record| owner.is_none_or(|owner| record.owner == owner))
        .collect();

    if let Some(mine) = matching.iter().find(|record| record.owner == subject) {
        return Ok(mine.clone());
    }
    let keys = state.meta.list_public_keys(repo).map_err(internal_error)?;
    let readable: Vec<&converge_model::SecretRecord> = matching
        .iter()
        .filter(|record| is_recipient(record, subject, &keys))
        .collect();
    match readable.as_slice() {
        [only] => Ok((*only).clone()),
        [] => Err(not_found_secret(name)),
        several => {
            let owners: Vec<&str> = several.iter().map(|r| r.owner.as_str()).collect();
            Err(bad_request(format!(
                "{name} is ambiguous: {} hold one; name whose with --owner",
                owners.join(", ")
            )))
        }
    }
}

/// Can `subject` decrypt this record, by owning it or holding one of
/// its recipient keys?
fn is_recipient(
    record: &converge_model::SecretRecord,
    subject: &str,
    keys: &[converge_model::PublicKeyRecord],
) -> bool {
    record.owner == subject
        || record.recipients.iter().any(|key_id| {
            keys.iter()
                .any(|key| &key.key_id == key_id && key.subject == subject)
        })
}

/// Names travel in a URL path and land in a database key, so the
/// grammar is narrow on purpose.
fn validate_secret_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.len() > 128 {
        return Err(bad_request("secret name must be 1-128 characters"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(bad_request(
            "secret name may use letters, digits, dash, underscore and dot",
        ));
    }
    Ok(())
}
