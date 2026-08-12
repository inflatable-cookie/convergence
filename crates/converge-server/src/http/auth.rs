//! Identity: provider exchange, tokens, and registered keys.

use std::sync::Arc;

use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;

use crate::authz::{Capability, authorize};

use super::{
    ApiError, DEFAULT_TOKEN_DAYS, SharedState, authorize_repo, bad_request, caller, forbidden,
    internal_error, mint_token, now_rfc3339, token_hash,
};

/// What a client needs to start a login, or the absence of one.
pub(crate) async fn auth_config(State(state): State<SharedState>) -> Json<serde_json::Value> {
    match &state.oidc {
        Some(verifier) => Json(serde_json::json!({
            "oidc": true,
            "issuer": verifier.issuer(),
            "client_id": verifier.audience(),
        })),
        None => Json(serde_json::json!({
            "oidc": false,
            "detail": "this server has no identity provider configured; \
                       use a token from `converge member add`",
        })),
    }
}

/// Exchange a verified identity token for a Convergence token.
///
/// Provisions the subject on first sight **with no grants**. Identity is
/// not authorization: an admin still decides what a person may do, and
/// the alternative — everyone in the directory becomes a member — is a
/// default nobody can afford.
pub(crate) async fn exchange_identity(
    State(state): State<SharedState>,
    Json(request): Json<converge_model::ExchangeIdentityRequest>,
) -> Result<Json<converge_model::TokenIssued>, ApiError> {
    let verifier = state
        .oidc
        .as_ref()
        .ok_or_else(|| bad_request("this server has no identity provider configured"))?;

    // Verification may fetch the issuer's keys, which blocks. Doing that
    // on the async worker builds a runtime inside a runtime and aborts
    // the connection mid-response, so it goes to a blocking thread.
    let verifier = Arc::clone(verifier);
    let id_token = request.id_token.clone();
    let subject = tokio::task::spawn_blocking(move || verifier.subject_from(&id_token))
        .await
        .map_err(|err| internal_error(anyhow::anyhow!("verify identity token: {err}")))?
        .map_err(|err| ApiError(StatusCode::UNAUTHORIZED, format!("{err:#}")))?;
    let verifier = state.oidc.as_ref().expect("checked above");

    state.meta.upsert_user(&subject).map_err(internal_error)?;

    let token = mint_token()?;
    let hash = token_hash(&token);
    let now = time::OffsetDateTime::now_utc();
    let expires_at = (now + time::Duration::days(DEFAULT_TOKEN_DAYS as i64))
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|err| internal_error(anyhow::anyhow!("format expiry: {err}")))?;
    let record = converge_model::TokenRecord {
        token_id: hash.chars().take(12).collect(),
        subject: subject.clone(),
        label: format!("sign-in via {}", verifier.issuer()),
        issued_at: now_rfc3339()?,
        issued_by: verifier.issuer().to_string(),
        repo_id: String::new(),
        expires_at,
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
    Ok(Json(converge_model::TokenIssued { token, record }))
}

/// Issue a token for the calling subject, optionally narrower than they
/// are (batch 21.2).
///
/// This is what makes doc 19 §10a a single command: an agent gets a
/// credential that cannot reach secrets, without needing a second
/// subject, its own membership, and its own lane.
pub(crate) async fn issue_token(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<converge_model::IssueTokenRequest>,
) -> Result<Json<converge_model::TokenIssued>, ApiError> {
    let caller = caller(&state, &headers)?;
    // Any member may mint a credential for themselves; narrowing is the
    // only thing on offer.
    authorize(
        state.meta.as_ref(),
        &caller.subject,
        &repo,
        "*",
        Capability::Read,
    )
    .map_err(|err| forbidden(format!("{err:#}")))?;

    let mut capabilities = Vec::new();
    for name in &request.capabilities {
        let capability = parse_capability(name)?;
        // Issuing must not widen. Both halves matter: the caller's own
        // token cannot mint past its scope, and no one can mint past
        // their grants.
        if !caller.permits(capability) {
            return Err(forbidden(format!(
                "your token does not carry {name}, so it cannot issue one that does"
            )));
        }
        authorize(state.meta.as_ref(), &caller.subject, &repo, "*", capability)
            .map_err(|_| forbidden(format!("you do not hold {name} in this repo")))?;
        capabilities.push(capability.as_str().to_string());
    }
    if capabilities.is_empty() {
        return Err(bad_request(
            "name at least one capability; an unscoped token is what `member add` issues",
        ));
    }

    let token = mint_token()?;
    let hash = token_hash(&token);
    let now = time::OffsetDateTime::now_utc();
    let days = request.expires_in_days.unwrap_or(DEFAULT_TOKEN_DAYS);
    let expires_at = if days == 0 {
        String::new()
    } else {
        (now + time::Duration::days(days as i64))
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|err| internal_error(anyhow::anyhow!("format expiry: {err}")))?
    };
    let record = converge_model::TokenRecord {
        token_id: hash.chars().take(12).collect(),
        subject: caller.subject.clone(),
        label: request.label,
        issued_at: now_rfc3339()?,
        issued_by: caller.subject.clone(),
        repo_id: repo.clone(),
        expires_at,
        last_used_at: String::new(),
        revoked_at: String::new(),
        revoked_by: String::new(),
        revoked_reason: String::new(),
        capabilities,
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
    Ok(Json(converge_model::TokenIssued { token, record }))
}

fn parse_capability(name: &str) -> Result<Capability, ApiError> {
    let known = [
        Capability::Read,
        Capability::SnapSync,
        Capability::Publish,
        Capability::Resolve,
        Capability::Approve,
        Capability::Promote,
        Capability::Release,
        Capability::Secret,
        Capability::Admin,
    ];
    known
        .into_iter()
        .find(|c| c.as_str() == name)
        .ok_or_else(|| bad_request(format!("unknown capability {name}")))
}

/// Tokens issued in this repo, as facts rather than credentials.
pub(crate) async fn list_tokens(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<converge_model::TokenRecord>>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Admin)?;
    Ok(Json(state.meta.list_tokens(&repo).map_err(internal_error)?))
}

/// Revoke by short id, with a reason. The record is kept: "revoked
/// when, by whom, and why" is what an incident asks, and a deleted row
/// answers none of it.
pub(crate) async fn revoke_token(
    State(state): State<SharedState>,
    Path((repo, token_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<converge_model::RevokeTokenRequest>,
) -> Result<Json<converge_model::TokenRecord>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Admin)?;
    let at = now_rfc3339()?;
    let record = state
        .meta
        .revoke_token(&token_id, &at, authz.subject(), &request.reason)
        .map_err(internal_error)?
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                format!("no live token {token_id} in this repo"),
            )
        })?;
    let _ = state.meta.add_event(
        &repo,
        "token.revoked",
        &format!("{}/{}", record.subject, record.token_id),
        &at,
    );
    Ok(Json(record))
}

/// Register a public key for the *calling* subject (batch 19.1).
///
/// The subject comes from the token, never the body. Letting a caller
/// name someone else would let them register a key that future secrets
/// get sealed to — the whole guarantee, given away in one field.
pub(crate) async fn register_key(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<converge_model::RegisterKeyRequest>,
) -> Result<Json<converge_model::PublicKeyRecord>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Read)?;
    // Parsed, not trusted: a malformed recipient would be stored and
    // then fail at encryption time, somewhere much less obvious.
    let recipient: age::x25519::Recipient = request
        .public_key
        .trim()
        .parse()
        .map_err(|err| bad_request(format!("not an age recipient: {err}")))?;
    let public_key = recipient.to_string();

    let record = converge_model::PublicKeyRecord {
        key_id: blake3::hash(public_key.as_bytes())
            .to_hex()
            .chars()
            .take(16)
            .collect(),
        subject: authz.subject().to_string(),
        public_key,
        label: request.label,
        created_at: now_rfc3339()?,
    };
    state
        .meta
        .add_public_key(&repo, &record)
        .map_err(internal_error)?;
    Ok(Json(record))
}

/// Every registered key in the repo. Public data: members need each
/// other's keys to share a secret, and hiding them would only mean an
/// out-of-band exchange that is easier to get wrong.
pub(crate) async fn list_keys(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<converge_model::PublicKeyRecord>>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    Ok(Json(
        state.meta.list_public_keys(&repo).map_err(internal_error)?,
    ))
}
