use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::{get, post};
use tokio::sync::RwLock;

use super::super::handlers_system::{bootstrap, healthz};
use super::super::identity_store::hash_token;
use super::super::persistence::load_repos_from_disk;
use super::super::routes::authed_router;
use super::super::types::{AccessToken, AppState, User};
use super::Args;

pub(super) fn build_state(
    args: &Args,
    users: HashMap<String, User>,
    tokens: HashMap<String, AccessToken>,
) -> Arc<AppState> {
    let default_user = users
        .values()
        .find(|u| u.admin)
        .or_else(|| users.values().next())
        .map(|u| u.handle.clone())
        .unwrap_or_else(|| "dev".to_string());

    let token_hash_index: HashMap<String, String> = tokens
        .values()
        .map(|t| (t.token_hash.clone(), t.id.clone()))
        .collect();

    Arc::new(AppState {
        default_user,
        data_dir: args.data_dir.clone(),
        repos: Arc::new(RwLock::new(HashMap::new())),
        users: Arc::new(RwLock::new(users)),
        tokens: Arc::new(RwLock::new(tokens)),
        token_hash_index: Arc::new(RwLock::new(token_hash_index)),
        bootstrap_token_hash: args.bootstrap_token.as_deref().map(hash_token),
    })
}

pub(super) async fn load_repos_into_state(state: &Arc<AppState>) -> Result<()> {
    let users = state.users.read().await;
    let handle_to_id: HashMap<String, String> = users
        .values()
        .map(|u| (u.handle.clone(), u.id.clone()))
        .collect();
    drop(users);

    // Best-effort load repos from disk so the dev server survives restarts.
    let loaded =
        load_repos_from_disk(state.as_ref(), &handle_to_id).context("load repos from disk")?;
    {
        let mut repos = state.repos.write().await;
        *repos = loaded;
    }
    Ok(())
}

pub(super) fn build_app_router(state: Arc<AppState>) -> Router {
    let authed = authed_router(state.clone());
    Router::new()
        .route("/healthz", get(healthz))
        .route("/bootstrap", post(bootstrap))
        .merge(authed)
        .with_state(state)
}
