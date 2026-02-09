//! Repo/gate/bundle/release/promotion administrative operations.

use std::collections::HashMap;

use anyhow::{Context, Result};

use super::{
    Bundle, CreateRepoRequest, GateGraph, GateGraphValidationError, Pins, Promotion, Publication,
    Release, RemoteClient, Repo,
};

mod bundle_ops;
mod release_promotion_gc;
mod repo_gate;
