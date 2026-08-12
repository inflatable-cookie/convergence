use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde_json::json;

use crate::authz::{AuthzContext, Capability, authorize};

use crate::storage::{AssociatingObjects, MetadataStore, ObjectStore};
use converge_model::{Page, WIRE_VERSION};

mod auth;
mod candidates;
mod content;
mod gates;
mod lanes;
mod members;
mod releases;
mod secrets;

use auth::{
    auth_config, exchange_identity, issue_token, list_keys, list_tokens, register_key, revoke_token,
};
use candidates::{
    approve, get_candidate, get_provenance, inbox, list_events, promote, publish, verify_candidate,
};
use content::{get_batch, get_object, negotiate, put_batch, put_object};
use gates::{create_repo, create_scope, get_gates, list_scopes, set_gates};
use lanes::{
    add_lane_member, create_lane, get_lane_head, get_snap, list_lanes, put_snap, set_lane_head,
};
use members::{add_member, list_members, remove_member};
use releases::{
    get_retention, list_releases, release, release_lookup, run_gc, set_retention, yank_release,
};
use secrets::{delete_secret, get_secret, list_secrets, set_secret};

pub struct AppState {
    pub meta: Arc<dyn MetadataStore>,
    pub objects: Arc<dyn ObjectStore>,
    /// token -> subject. Slice-grade identity (arch 14 notes MVP tokens);
    /// real identity arrives with a later roadmap.
    pub tokens: HashMap<String, String>,
    /// Held for the duration of a GC run so a second one is refused
    /// rather than repeating the walk (batch 14.4).
    pub gc_running: Arc<tokio::sync::Mutex<()>>,
    /// Trusted identity provider, when one is configured (batch 21.3).
    pub oidc: Option<Arc<crate::oidc::OidcVerifier>>,
}

type SharedState = Arc<AppState>;

const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Page cap for cursor listings (batch 15.2), matching the event feed's.
const MAX_PAGE_ITEMS: usize = 1000;

pub fn router(state: AppState) -> Router {
    let shared: SharedState = Arc::new(state);
    Router::new()
        .route("/api/healthz", get(healthz))
        .route("/api/auth/config", get(auth_config))
        .route("/api/auth/exchange", post(exchange_identity))
        .route("/api/repos/:repo/negotiate", post(negotiate))
        .route(
            "/api/repos/:repo/objects/:kind/:id",
            put(put_object).get(get_object),
        )
        .route("/api/repos/:repo/objects/batch", post(put_batch))
        .route("/api/repos/:repo/objects/batch-get", post(get_batch))
        .route("/api/publish", post(publish))
        .route("/api/candidates/:id", get(get_candidate))
        .route("/api/candidates/:id/provenance", get(get_provenance))
        .route("/api/candidates/:id/verify", get(verify_candidate))
        .route("/api/candidates/:id/approve", post(approve))
        .route("/api/candidates/:id/promote", post(promote))
        .route("/api/repos", post(create_repo))
        .route(
            "/api/repos/:repo/members",
            post(add_member).get(list_members),
        )
        .route("/api/repos/:repo/members/:subject", delete(remove_member))
        .route("/api/repos/:repo/keys", post(register_key).get(list_keys))
        .route(
            "/api/repos/:repo/tokens",
            post(issue_token).get(list_tokens),
        )
        .route(
            "/api/repos/:repo/tokens/:token_id/revoke",
            post(revoke_token),
        )
        .route("/api/repos/:repo/secrets", get(list_secrets))
        .route(
            "/api/repos/:repo/secrets/:name",
            put(set_secret).get(get_secret).delete(delete_secret),
        )
        .route(
            "/api/repos/:repo/scopes",
            post(create_scope).get(list_scopes),
        )
        .route("/api/repos/:repo/lanes", post(create_lane).get(list_lanes))
        .route(
            "/api/repos/:repo/lanes/:lane/members",
            post(add_lane_member),
        )
        .route("/api/repos/:repo/snaps/:id", put(put_snap).get(get_snap))
        .route("/api/repos/:repo/lane-head", post(set_lane_head))
        .route("/api/repos/:repo/lane-head/:lane", get(get_lane_head))
        .route("/api/repos/:repo/gates", get(get_gates).put(set_gates))
        .route("/api/repos/:repo/inbox", get(inbox))
        .route("/api/repos/:repo/events", get(list_events))
        .route("/api/candidates/:id/release", post(release))
        .route("/api/repos/:repo/releases", get(list_releases))
        .route("/api/repos/:repo/release/:version", get(release_lookup))
        .route("/api/repos/:repo/release/:version/yank", post(yank_release))
        .route(
            "/api/repos/:repo/retention",
            get(get_retention).put(set_retention),
        )
        .route("/api/repos/:repo/gc", post(run_gc))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(axum::middleware::from_fn_with_state(
            shared.clone(),
            require_authentication,
        ))
        .with_state(shared)
}

/// Routes that must work without a credential.
///
/// Health is for load balancers; the two auth routes are how a client
/// *gets* a credential, and `/api/auth/exchange` carries an identity
/// token from the provider rather than one of ours.
const PUBLIC_ROUTES: &[&str] = &["/api/healthz", "/api/auth/config", "/api/auth/exchange"];

/// Authenticate before routing, not inside each handler (batch 21.4).
///
/// Handlers still authenticate — they need the subject, and a check
/// that only exists in a layer is one route registration away from
/// being skipped. What this adds is *ordering*: without it, axum runs
/// the `Json` extractor first, so an anonymous caller reaches the body
/// parser on every route, learns the request schema from a 422, and can
/// push a body up to the size limit through it before anyone asks who
/// they are.
async fn require_authentication(
    State(state): State<SharedState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if PUBLIC_ROUTES.contains(&request.uri().path()) {
        return next.run(request).await;
    }
    // Only authentication belongs here. Authorization needs the repo and
    // the operation, which is a per-handler question.
    if let Err(error) = subject(&state, request.headers()) {
        return error.into_response();
    }
    next.run(request).await
}

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"ok": false, "error": self.1}))).into_response()
    }
}

fn bad_request(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

fn forbidden(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::FORBIDDEN, msg.into())
}

/// Storage-layer failure (batch 11.3): the chain is logged server-side;
/// only a stable public message crosses the wire.
fn internal_error(err: anyhow::Error) -> ApiError {
    eprintln!("internal error: {err:#}");
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
}

/// Who is calling, and how narrow their credential is (batch 21.2).
#[derive(Clone, Debug)]
struct Caller {
    subject: String,
    /// Empty means unscoped: the subject's full set.
    scope: Vec<String>,
}

impl Caller {
    /// Does this credential permit `capability`?
    ///
    /// Checked with the same implication rules `authorize` uses, so a
    /// scope cannot disagree with a grant about what implies what — a
    /// token scoped to `admin` really is total, and one scoped to
    /// `publish` really does cover snap-sync.
    fn permits(&self, capability: Capability) -> bool {
        if self.scope.is_empty() {
            return true;
        }
        crate::authz::satisfying_capabilities(capability)
            .iter()
            .any(|c| self.scope.iter().any(|held| held == c.as_str()))
    }
}

fn caller(state: &AppState, headers: &HeaderMap) -> Result<Caller, ApiError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "missing bearer token".into()))?;
    if let Some(subject) = state.tokens.get(token) {
        return Ok(Caller {
            subject: subject.clone(),
            scope: Vec::new(),
        });
    }
    let record = state
        .meta
        .token_by_hash(&token_hash(token))
        .map_err(internal_error)?;
    let subject = subject(state, headers)?;
    Ok(Caller {
        subject,
        scope: record.map(|r| r.capabilities).unwrap_or_default(),
    })
}

fn subject(state: &AppState, headers: &HeaderMap) -> Result<String, ApiError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "missing bearer token".into()))?;
    // Startup-flag tokens first (dev), then tokens issued at runtime
    // (batch 16.3). Issued tokens are stored hashed, so the comparison is
    // over the hash and the database never holds a usable credential.
    if let Some(subject) = state.tokens.get(token) {
        return Ok(subject.clone());
    }

    let hash = token_hash(token);
    let record = state
        .meta
        .token_by_hash(&hash)
        .map_err(internal_error)?
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "unknown token".into()))?;

    // Revoked and expired are different problems for whoever is holding
    // it, so they get different answers (batch 21.1).
    if !record.revoked_at.is_empty() {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            format!(
                "token revoked {}{}",
                record.revoked_at,
                if record.revoked_reason.is_empty() {
                    String::new()
                } else {
                    format!(": {}", record.revoked_reason)
                }
            ),
        ));
    }
    let now = now_rfc3339()?;
    if !record.expires_at.is_empty() && record.expires_at.as_str() <= now.as_str() {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            format!("token expired {}; ask for a new one", record.expires_at),
        ));
    }

    // Coarse last-used tracking: to the day, because the value of this
    // field is "is anyone still using this token", and a write per
    // request to answer that would be a poor trade.
    if record.last_used_at.get(..10) != now.get(..10) {
        let _ = state.meta.touch_token(&hash, &now);
    }
    Ok(record.subject)
}

/// Tokens are recognised by hash, never stored raw.
pub fn token_hash(token: &str) -> String {
    blake3::hash(token.as_bytes()).to_hex().to_string()
}

/// Server-wide admin check for operations that name no repo yet.
fn site_admin(state: &AppState, headers: &HeaderMap) -> Result<String, ApiError> {
    let caller = caller(state, headers)?;
    // Scope applies here too: creating repos is the widest thing there
    // is, and a token that cannot administer a repo it knows about
    // should not be able to make new ones.
    if !caller.permits(Capability::Admin) {
        return Err(forbidden(format!(
            "this token is scoped to {} and does not carry admin",
            caller.scope.join(", ")
        )));
    }
    if state
        .meta
        .is_site_admin(&caller.subject)
        .map_err(internal_error)?
    {
        return Ok(caller.subject);
    }
    Err(forbidden(format!(
        "{} is not a server admin",
        caller.subject
    )))
}

fn check_wire_version(version: u32) -> Result<(), ApiError> {
    if version != WIRE_VERSION {
        return Err(bad_request(format!(
            "unsupported wire version {version} (server speaks {WIRE_VERSION})"
        )));
    }
    Ok(())
}

async fn healthz() -> Json<serde_json::Value> {
    Json(json!({"ok": true}))
}

/// Repo-scoped object view (batch 11.1): writes record the object→repo
/// association; `has` answers for this repo only.
fn scoped_objects<'a>(state: &'a AppState, repo: &str) -> AssociatingObjects<'a> {
    AssociatingObjects {
        inner: state.objects.as_ref(),
        meta: state.meta.as_ref(),
        repo_id: repo.to_string(),
    }
}

/// The single authorization entry point: scope, then grant.
///
/// Batch 21.2 added the scope check here and left handlers that called
/// `authorize` directly untouched, which made scope a property of the
/// routes it happened to reach rather than of the token — `add_member`
/// among them, so a read-scoped credential could grant itself admin.
/// Batch 21.4 routes everything through this, and `authorize` is no
/// longer called from a handler.
fn authorize_scoped(
    state: &AppState,
    headers: &HeaderMap,
    repo: &str,
    scope: &str,
    capability: Capability,
) -> Result<AuthzContext, ApiError> {
    let caller = caller(state, headers)?;
    // Scope before grant (batch 21.2): a narrow token must be refused
    // even when its subject would be allowed, and the refusal should say
    // which of the two it was.
    if !caller.permits(capability) {
        return Err(forbidden(format!(
            "this token is scoped to {} and does not carry {}",
            caller.scope.join(", "),
            capability.as_str()
        )));
    }
    authorize(
        state.meta.as_ref(),
        &caller.subject,
        repo,
        scope,
        capability,
    )
    .map_err(|err| forbidden(format!("{err:#}")))
}

fn authorize_repo(
    state: &AppState,
    headers: &HeaderMap,
    repo: &str,
    capability: Capability,
) -> Result<AuthzContext, ApiError> {
    authorize_scoped(state, headers, repo, "*", capability)
}

/// Default lifetime for an issued token (batch 21.1).
const DEFAULT_TOKEN_DAYS: u32 = 90;

fn now_rfc3339() -> Result<String, ApiError> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|err| internal_error(anyhow::anyhow!("format timestamp: {err}")))
}

/// 256 bits from the OS CSPRNG, hex encoded. The server keeps only the
/// hash, so this string exists exactly once: in the response that issued
/// it.
fn mint_token() -> Result<String, ApiError> {
    mint_admin_token().map_err(internal_error)
}

/// The same minting the API uses, for the bootstrap path in `main`.
pub fn mint_admin_token() -> anyhow::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| anyhow::anyhow!("read system randomness: {err}"))?;
    Ok(blake3::Hasher::new()
        .update(&bytes)
        .finalize()
        .to_hex()
        .to_string())
}

/// Cursor paging params (batch 15.2). `limit` is clamped to the page cap
/// whether or not the client sends one.
#[derive(serde::Deserialize)]
struct PageParams {
    #[serde(default)]
    after: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

impl PageParams {
    fn limit(&self) -> usize {
        self.limit
            .unwrap_or(MAX_PAGE_ITEMS)
            .clamp(1, MAX_PAGE_ITEMS)
    }
}

/// Build a page, reporting a cursor only when the page filled — a short
/// page means the listing is exhausted.
fn page_of<T>(items: Vec<T>, limit: usize, cursor: impl Fn(&T) -> String) -> Page<T> {
    let next_cursor = (items.len() == limit)
        .then(|| items.last().map(&cursor))
        .flatten();
    Page { items, next_cursor }
}
