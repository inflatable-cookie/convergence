//! Repo administration: gates, scopes, and repo creation.

use axum::Json;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;

use converge_model::Page;

use crate::authz::Capability;

use super::{
    ApiError, PageParams, SharedState, authorize_repo, bad_request, internal_error, now_rfc3339,
    page_of, site_admin,
};

/// Create a repo with its `default` scope and an `intake` gate (batch
/// 16.3). Server admins only: this runs before the repo exists, so there
/// is no repo-scoped grant to check.
pub(crate) async fn create_repo(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<converge_model::CreateRepoRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let admin = site_admin(&state, &headers)?;
    if request.repo_id.is_empty() || request.repo_id == "*" {
        return Err(bad_request(format!(
            "invalid repo id {:?}",
            request.repo_id
        )));
    }
    if state
        .meta
        .repo_exists(&request.repo_id)
        .map_err(internal_error)?
    {
        return Err(bad_request(format!("repo {} exists", request.repo_id)));
    }
    let created_at = now_rfc3339()?;
    state
        .meta
        .create_repo(&request.repo_id)
        .map_err(internal_error)?;
    state
        .meta
        .create_scope(&request.repo_id, "default", &created_at)
        .map_err(internal_error)?;
    // A repo with no gate cannot accept a publish, so an empty one would
    // be a second setup step nobody is told about.
    state
        .meta
        .set_gate_graph(
            &request.repo_id,
            &converge_model::GateGraph {
                gates: vec![converge_model::GateNode {
                    gate_id: "intake".into(),
                    name: "Intake".into(),
                    upstreams: vec![],
                    required_approvals: 0,
                    strategy: "whole-file".into(),
                    may_release: true,
                }],
            },
        )
        .map_err(internal_error)?;
    // The creator can work in it immediately.
    for capability in [
        Capability::Read,
        Capability::Publish,
        Capability::Resolve,
        Capability::Approve,
        Capability::Promote,
        Capability::Release,
        Capability::Admin,
    ] {
        state
            .meta
            .add_grant(&admin, &request.repo_id, "*", capability.as_str())
            .map_err(internal_error)?;
    }
    Ok(Json(serde_json::json!({
        "repo_id": request.repo_id,
        "scope": "default",
        "gate": "intake",
    })))
}

/// The repo's gate graph (batch 17.1). Reading the shape of the pipeline
/// you publish into needed a server round trip that did not exist.
pub(crate) async fn get_gates(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<converge_model::GateGraph>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    Ok(Json(
        state.meta.get_gate_graph(&repo).map_err(internal_error)?,
    ))
}

/// Replace a repo's gate graph (batch 26.2).
///
/// Three things stand between a request and the write, in this order,
/// because each is cheaper than the next and a caller deserves the most
/// specific refusal available:
///
/// 1. is the graph legal at all (26.1 validation)
/// 2. would the change strand work that exists
/// 3. is it still the graph the caller read
///
/// The `force` escape hatch is deliberate. Refusing outright sounds
/// safer and is not: a repo whose graph can never be reshaped because it
/// once held a publication is a repo that has to be recreated, which is
/// worse than a documented sharp edge. Batch 20.4 settled this shape for
/// rotation-after-departure — warn, name the consequence, let the
/// operator decide, and never make the safe path the impossible one.
///
/// What this does *not* claim: a publish that has already resolved its
/// target gate may complete under the previous graph. Closing that would
/// mean asserting the graph inside the publish batch, on the hot path,
/// to defend against a reshape that is refused anyway whenever the gate
/// has work in it.
pub(crate) async fn set_gates(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<converge_model::SetGatesRequest>,
) -> Result<Json<converge_model::SetGatesResponse>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Admin)?;

    let proposed = converge_model::GateGraph {
        gates: request.gates,
    };
    let faults = converge_model::gates::validate(&proposed);
    if !faults.is_empty() {
        return Err(bad_request(
            faults
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }

    let current = state.meta.get_gate_graph(&repo).map_err(internal_error)?;
    let occupancy = state.meta.gate_occupancy(&repo).map_err(internal_error)?;
    let impact = converge_model::gates::impact_of(&current, &proposed, &occupancy);

    if request.dry_run {
        return Ok(Json(converge_model::SetGatesResponse {
            applied: false,
            impact,
        }));
    }
    if impact.strands_work() && !request.force {
        let stranded = impact
            .occupancy
            .iter()
            .filter(|o| !o.is_empty())
            .map(|o| {
                format!(
                    "{} ({} candidate(s), {} open publication(s))",
                    o.gate_id, o.candidates, o.open_publications
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ApiError(
            StatusCode::CONFLICT,
            format!(
                "this change would strand work in {stranded}. That work stays in \
                 the store either way, but nothing would address it. Promote or \
                 release it first, or resend with force."
            ),
        ));
    }

    let created_at = now_rfc3339()?;
    let mut ops = Vec::new();
    // Only when the caller said what they read. Omitting it is allowed —
    // a script setting a known graph should not have to round-trip — but
    // then a concurrent edit is lost rather than reported.
    if let Some(expected) = request.expected {
        ops.push(crate::storage::MetaOp::AssertGateGraph {
            repo_id: repo.clone(),
            expected,
        });
    }
    ops.push(crate::storage::MetaOp::SetGateGraph {
        repo_id: repo.clone(),
        graph: proposed,
    });
    // In the same batch as the write: another workspace learning about a
    // reshape that did not happen would be worse than not learning.
    ops.push(crate::storage::MetaOp::AddEvent {
        repo_id: repo.clone(),
        kind: "gate.changed".into(),
        subject_id: repo.clone(),
        created_at,
    });
    state
        .meta
        .apply_batch(&ops)
        .map_err(|err| ApiError(StatusCode::CONFLICT, format!("{err}")))?;

    Ok(Json(converge_model::SetGatesResponse {
        applied: true,
        impact,
    }))
}

/// Register a scope (batch 14.3). Admin-only: scopes define the
/// partitioning of a repo, so minting them is a policy act.
pub(crate) async fn create_scope(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<converge_model::CreateScopeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Admin)?;
    if request.scope_id.is_empty() || request.scope_id == "*" {
        return Err(bad_request(format!(
            "invalid scope id {:?}",
            request.scope_id
        )));
    }
    let created_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|err| internal_error(anyhow::anyhow!("format timestamp: {err}")))?;
    state
        .meta
        .create_scope(&repo, &request.scope_id, &created_at)
        .map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub(crate) async fn list_scopes(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    Query(params): Query<PageParams>,
    headers: HeaderMap,
) -> Result<Json<Page<String>>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    let limit = params.limit();
    let scopes = state
        .meta
        .list_scopes_page(&repo, params.after.as_deref(), limit)
        .map_err(internal_error)?;
    Ok(Json(page_of(scopes, limit, |s| s.clone())))
}
