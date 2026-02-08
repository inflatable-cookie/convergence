//! Authenticated HTTP route registration for the converge server.

use super::handlers_system::require_bearer;
use super::*;

pub(super) fn authed_router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/whoami", get(whoami))
        .route("/users", get(list_users).post(create_user))
        .route(
            "/users/:user_id/tokens",
            axum::routing::post(create_token_for_user),
        )
        .route("/tokens", get(list_tokens).post(create_token))
        .route(
            "/tokens/:token_id/revoke",
            axum::routing::post(revoke_token),
        )
        .route(
            "/repos/:repo_id/members",
            get(list_repo_members).post(add_repo_member),
        )
        .route(
            "/repos/:repo_id/members/:handle",
            axum::routing::delete(remove_repo_member),
        )
        .route(
            "/repos/:repo_id/lanes/:lane_id/members",
            get(list_lane_members).post(add_lane_member),
        )
        .route(
            "/repos/:repo_id/lanes/:lane_id/members/:handle",
            axum::routing::delete(remove_lane_member),
        )
        .route("/repos", get(list_repos).post(create_repo))
        .route("/repos/:repo_id", get(get_repo))
        .route("/repos/:repo_id/permissions", get(get_repo_permissions))
        .route("/repos/:repo_id/lanes", get(list_lanes))
        .route(
            "/repos/:repo_id/lanes/:lane_id/heads/me",
            axum::routing::post(update_lane_head_me),
        )
        .route(
            "/repos/:repo_id/lanes/:lane_id/heads/:user",
            get(get_lane_head),
        )
        .route("/repos/:repo_id/gates", get(list_gates))
        .route(
            "/repos/:repo_id/gate-graph",
            get(get_gate_graph).put(put_gate_graph),
        )
        .route(
            "/repos/:repo_id/scopes",
            get(list_scopes).post(create_scope),
        )
        .route(
            "/repos/:repo_id/publications",
            get(list_publications).post(create_publication),
        )
        .route(
            "/repos/:repo_id/bundles",
            get(list_bundles).post(create_bundle),
        )
        .route("/repos/:repo_id/bundles/:bundle_id", get(get_bundle))
        .route(
            "/repos/:repo_id/bundles/:bundle_id/pin",
            axum::routing::post(pin_bundle),
        )
        .route(
            "/repos/:repo_id/bundles/:bundle_id/unpin",
            axum::routing::post(unpin_bundle),
        )
        .route("/repos/:repo_id/pins", get(list_pins))
        .route(
            "/repos/:repo_id/bundles/:bundle_id/approve",
            axum::routing::post(approve_bundle),
        )
        .route(
            "/repos/:repo_id/releases",
            get(list_releases).post(create_release),
        )
        .route(
            "/repos/:repo_id/releases/:channel",
            get(get_release_channel),
        )
        .route(
            "/repos/:repo_id/promotions",
            get(list_promotions).post(create_promotion),
        )
        .route("/repos/:repo_id/promotion-state", get(get_promotion_state))
        .route(
            "/repos/:repo_id/objects/blobs/:blob_id",
            axum::routing::put(put_blob).get(get_blob),
        )
        .route(
            "/repos/:repo_id/objects/manifests/:manifest_id",
            axum::routing::put(put_manifest).get(get_manifest),
        )
        .route(
            "/repos/:repo_id/objects/recipes/:recipe_id",
            axum::routing::put(put_recipe).get(get_recipe),
        )
        .route(
            "/repos/:repo_id/objects/snaps/:snap_id",
            axum::routing::put(put_snap).get(get_snap),
        )
        .route(
            "/repos/:repo_id/objects/missing",
            axum::routing::post(find_missing_objects),
        )
        .route("/repos/:repo_id/gc", axum::routing::post(gc_repo))
        .layer(middleware::from_fn_with_state(state, require_bearer))
}
