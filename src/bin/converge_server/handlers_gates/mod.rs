use super::*;

mod gate_graph;
mod scopes;

pub(crate) use self::gate_graph::{get_gate_graph, list_gates, put_gate_graph};
pub(crate) use self::scopes::{create_scope, list_scopes};

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreateScopeRequest {
    pub(crate) id: String,
}
