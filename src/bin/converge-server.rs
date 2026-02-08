#![allow(clippy::result_large_err)]

use std::path::PathBuf;
use std::sync::Arc;
use std::{collections::HashMap, collections::HashSet};

use anyhow::{Context, Result};
use axum::extract::{Extension, Query, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router, extract::Path};
use tokio::sync::RwLock;

#[path = "converge_server/persistence.rs"]
mod persistence;
use self::persistence::*;
#[path = "converge_server/identity_store.rs"]
mod identity_store;
use self::identity_store::*;
#[path = "converge_server/access.rs"]
mod access;
use self::access::*;
#[path = "converge_server/validators.rs"]
mod validators;
use self::validators::*;
#[path = "converge_server/http_error.rs"]
mod http_error;
use self::http_error::*;
#[path = "converge_server/gate_graph_validation.rs"]
mod gate_graph_validation;
use self::gate_graph_validation::*;
#[path = "converge_server/handlers_identity.rs"]
mod handlers_identity;
use self::handlers_identity::*;
#[path = "converge_server/handlers_repo.rs"]
mod handlers_repo;
#[path = "converge_server/handlers_system.rs"]
mod handlers_system;
use self::handlers_repo::*;
#[path = "converge_server/handlers_gates.rs"]
mod handlers_gates;
use self::handlers_gates::*;
#[path = "converge_server/handlers_objects.rs"]
mod handlers_objects;
use self::handlers_objects::*;
#[path = "converge_server/handlers_publications/mod.rs"]
mod handlers_publications;
use self::handlers_publications::*;
#[path = "converge_server/handlers_release.rs"]
mod handlers_release;
use self::handlers_release::*;
#[path = "converge_server/handlers_gc.rs"]
mod handlers_gc;
use self::handlers_gc::*;
#[path = "converge_server/object_graph/mod.rs"]
mod object_graph;
use self::object_graph::*;
#[path = "converge_server/routes.rs"]
mod routes;
#[path = "converge_server/types.rs"]
mod types;
use self::types::*;
#[path = "converge_server/runtime.rs"]
mod runtime;
use self::runtime::run;

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{:#}", err);
        std::process::exit(1);
    }
}
